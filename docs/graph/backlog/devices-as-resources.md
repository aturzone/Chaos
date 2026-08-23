# Devices as resources — one model, many machines

**Atur's words**: *"multi device for run a same model but use just in 1 device …
a mode for use resources of other devices for main runner device, and other
devices in a same network help main device to use a model faster"*.

This is the most promising idea on the roadmap, because it attacks the exact
thing that limits this project. It is also the one where the arithmetic must be
done **before** the code, because the obvious version of it loses.

## Why it fits Chaos specifically

Chaos's problem is not FLOPS. It is that **3.3 GB must be read per token** on
V4-Flash and a token costs **2.4 s**: 1.56 s of expert reads plus 0.84 s of
compute that never touches the disk. The frontier is memory, and other machines
on a LAN have memory.

The shape of the model makes this unusually favourable:

| V4-Flash | |
|---|---|
| hidden (`embedding_length`) | **4096** |
| blocks | **43** |
| experts per layer | **256** |
| experts used per token | **6** |
| expert FFN length | 2048 |

## The number that decides it

**Activations are tiny and weights are enormous.** A hidden state is
`4096 × 4 bytes = 16 KB`. Everything follows from that.

| what moves | per token | over 1 GbE (125 MB/s) |
|---|---|---|
| one layer boundary (pipeline) | 16 KB | 0.13 ms |
| all 43 boundaries | 688 KB | **5.5 ms** |
| expert-parallel: 6 experts × 43 layers, there and back | ~8 MB | **66 ms** |
| the expert weights themselves | 3.3 GB | 26 s — **never do this** |

Against a token that currently costs **2400 ms**, moving activations costs
**0.2%–3%**. Moving weights costs eleven times the whole token.

**So: send the work to the weights, never the weights to the work.** That single
rule decides every design question below.

## What actually helps, in order

### 1. Expert-parallel MoE over the LAN — the big one

Each device holds a slice of the 256 experts per layer **in RAM**. The main
device routes a token's hidden state to whichever device holds each of the 6
chosen experts; that device computes and returns 16 KB.

- Replaces ~1560 ms of disk read with ~66 ms of network.
- Needs the experts to fit in **pooled** RAM: 144 GB across all devices.
- MoE is the ideal case — only 6 of 256 experts are touched, so each device is
  idle most of the time and its RAM is doing the work, which is the point.

**Honest ceiling**: the measured frontier is 16 GB → 0.42 tok/s, 64 → 0.55,
128 → 0.93, 160 → 1.19. Pooling RAM moves along *that* curve, so full residency
across devices lands near **1.19 tok/s** — because 0.84 s per token never
touches the disk and pooling memory does nothing for it.

### 2. Tensor-parallel compute — the other half

Splitting a single matmul across devices is what attacks the 0.84 s. It needs an
all-reduce per layer: 43 layers × 2 syncs, latency-bound rather than
bandwidth-bound, so roughly **43 ms** on a quiet LAN.

With four comparable machines: 0.84 s → ~0.21 s + 0.05 s of network.
Combined with (1), a token lands near **0.3 s ≈ 3.3 tok/s**.

**Write this down plainly: four machines get single-digit tok/s on V4-Flash, not
20.** 20 tok/s needs 67.7 GB/s to the experts, which is a GPU-memory
specification and no number of gigabit links reaches it. Distributed makes big
models *usable*; it does not make them fast.

### 3. Pipeline-parallel — throughput, not latency

Each device owns a run of layers. Cheap (5.5 ms/token) and simple, but for a
*single* token stream it is sequential: it improves tokens-per-second across
concurrent requests, not the latency of one. Worth having for `chaos-serve` with
several clients; worth nobody's hope for a single chat.

### 4. A phone as a *client*, never as a worker

A phone on Wi-Fi has high, variable latency and its battery is a real cost. It
belongs on the other end of the OpenAI API, not inside the layer loop.

## What loses, so nobody tries it twice

- **Network RAM as a block cache.** 1 GbE is 125 MB/s; this machine's NVMe is
  **2740 MB/s**. Serving expert weights over the network is *20x slower than the
  disk we already have*. Only worth reconsidering where local storage is slower
  than the link — a phone, or a machine with a spinning disk.
- **Shipping weights to idle compute.** 3.3 GB per token, 26 s on 1 GbE. See the
  table.
- **Splitting a model across the internet.** Every number above assumes a quiet
  LAN with sub-millisecond RTT. Over WAN, the 43 all-reduces dominate and the
  whole idea inverts.

## Design sketch

```
chaos-serve --workers 192.168.1.20,192.168.1.21     the main device
chaos-worker --bind 0.0.0.0:8232                    on each helper
```

- **A worker holds weights and answers with activations.** It is `chaos-run`
  without a token loop: load an assigned slice, wait for a hidden state, return
  one. It should refuse to hold a slice larger than its free memory, and say so.
- **The main device owns routing, sampling and the KV cache.** Workers stay
  stateless per token, so a worker that dies is a slowdown, not a corruption:
  fall back to reading that expert from local disk.
- **Assignment comes from the probe.** Each worker reports RAM, cores and link
  speed; the main device solves a simple bin-pack over experts, weighted by how
  often each is routed to. `core/plan` already scores residency policies — the
  same scoring, with device identity added.
- **Announce over mDNS-ish UDP broadcast**, because typing IP addresses is the
  kind of friction that stops a feature being used.

## Definition of done

- Two machines on one LAN generate from V4-Flash with the second holding half
  the experts, and the **tok/s is measured against the same machine alone**,
  alternating in one session — the project's standing rule for any comparison.
- Killing a worker mid-generation degrades to local reads and finishes.
- `chaos-probe` on the main device lists discovered workers and what each would
  contribute, **before** anything is loaded.
- The measured numbers go in a research node. **No claim of a speed-up exists
  until then**, and the expected result is single-digit tok/s, not 20.

## Order of work

1. `chaos-worker`: load a slice, answer with activations. Local loopback first —
   two processes on one machine prove the protocol without a second machine.
2. Expert-parallel routing on the main device, with local-disk fallback.
3. Measurement against the single-machine baseline. **Stop here and report.**
4. Discovery and assignment from the probe.
5. Tensor-parallel, only if step 3's numbers justify the complexity.
