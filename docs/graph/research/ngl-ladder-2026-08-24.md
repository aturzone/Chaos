# The `-ngl` ladder: 1.77x prefill, 0.46x generation, and `--auto` picks wrong (2026-08-24)

`ngl-partial-offload-2026-08-16.md` settled the accuracy question — the device
path chooses the same token as the CPU on 8 of 8 parity prompts — and left this
open in writing:

> **A speed number for `-ngl`.** Nothing here is a performance claim. The
> partial-offload tok/s ladder against resident VRAM is the interesting
> measurement and it has not been run.

It has now. `scripts/ngl-ladder.ps1`.

## The machine, the model and the command

RTX 3050 6 GB Laptop (5.11 GiB free) against a 20-thread i7-13650HX.
**Qwen3-4B-Q4_K_M, 2.32 GiB resident — it fits in VRAM entirely**, so this is
the favourable case, not a streaming one.

```
chaos-run Qwen3-4B -f <1080-token prompt> -n 32 --device 1 -ngl <N>
```

Best of two per row, one session, nothing else running.

## The ladder

```
   ngl  prefill tok/s     gen tok/s    vs ngl 0
     0         77.52          6.39       1.00x     <- CPU only
     8         81.53          3.38       0.53x
    16         97.68          2.84       0.44x
    24        125.75          4.13       0.65x
    32        119.58          3.46       0.54x
    99        137.58          2.92       0.46x     <- all 36 blocks on the card
```

**Prefill: 1.77x.** Monotone apart from a wobble at 24–32 that is inside the
run-to-run spread. This is the compute-bound phase and the GPU wins it, which is
what a GPU is for.

**Generation: never once faster than the CPU, and 2.2x slower fully resident.**
Not a streaming case, not a cache miss — 2.32 GiB sitting in 5.11 GiB of VRAM,
read from VRAM every token.

**That last row deserves suspicion rather than a shrug.** An RTX 3050's memory
is roughly 2x this laptop's DDR5, so a bandwidth-bound phase reading resident
weights should be *faster* on the card, not 2.2x slower. Whatever is costing the
time is per-token overhead — synchronisation, graph splits, activations crossing
the bus at layer boundaries — and it is a fact about **this device path**, not
about the hardware. **Do not quote this as "the GPU is slower at generation".**
It is "Chaos's Vulkan path is slower at generation here, by 2.2x, and nobody has
looked at why".

## Where the GPU stops paying

From the two ends of the ladder: the GPU saves **5.63 ms per prompt token** and
loses **185.97 ms per generated token**.

```
break-even prompt:generation ratio = 33 : 1
```

```
prompt  1080, generate   16: CPU   16.4s  GPU   13.3s  -> GPU by 19%
prompt  1080, generate  200: CPU   45.2s  GPU   76.3s  -> CPU by 41%
prompt  1080, generate  512: CPU   94.1s  GPU  183.2s  -> CPU by 49%
prompt   200, generate  200: CPU   33.9s  GPU   69.9s  -> CPU by 52%
prompt  4000, generate  200: CPU   82.9s  GPU   97.6s  -> CPU by 15%
```

**A prompt has to be 33 times longer than the answer before the card is worth
using.** Summarising a long document qualifies. A chat turn does not; neither
does anything that writes code.

## `--auto` decides on fit, and fit is the wrong question

```
$ chaos-run Qwen3-4B -f <1080 tokens> -n 16 --auto
device     1 (Vulkan1): the whole model is 2.3 GiB against 4.1 GiB usable
                        VRAM -- offloading all of it
prefill    1080 tokens in 9.1s (118.11 tok/s)
generated  16 tokens in 3.7s (4.34 tok/s)
```

It is choosing on **"does it fit"**. At `-n 16` that happens to be right, by
19%. At `-n 200` — an ordinary reply — **the same decision is 41% slower than
leaving the GPU alone**, and `--auto` had `-n` on its own command line the whole
time.

**Not fixed here, deliberately.** The rule is one line, and one line derived
from one model on one machine is how this project has been wrong before. What
would make it safe is the ratio measured per machine, the way `--auto` already
measures threads — and that measurement does not exist. Filed rather than
guessed.

## What the instrument cost, because it is the same lesson three times

Three separate runs produced numbers that looked fine and were not:

1. **`2>&1` on a native command.** In Windows PowerShell that wraps every stderr
   line in an ErrorRecord, so ggml's informational `Found 2 Vulkan devices:`
   banner became a terminating error under `$ErrorActionPreference = 'Stop'`.
   The script uses `--log-file` now, which is what it is for.
2. **A fixed log path.** A run that wrote nothing left the *previous* run's file
   in place and `Get-Content` reported last time's timings as this time's. That
   is how a 1080-token prefill came back byte-identical to a 5-token one, and it
   would have been published as "prompt length makes no difference". The log is
   deleted before every run and a missing one is now a failed row.
3. **Two ladders in one file.** `chaos-run` was killed but not its parent shell,
   so an old ladder kept running and interleaved with the new one. Only
   `FileOpenFailure` in the transcript gave it away.

And a fourth, upstream of all three: **the first ladder measured prefill from a
five-token prompt** — 0.2 s of work — where the number is overhead, not
throughput. The same model on the same CPU reads 20.73 tok/s on five tokens and
77.52 on 1080.

## Open

1. **Why is resident-in-VRAM generation 2.2x slower than CPU?** The bandwidth
   argument says it should be about 2x faster. Until that is understood, the
   GPU tier's generation number is a symptom, not a specification.
2. **The same ladder on a model that does not fit**, where partial offload is
   the only option rather than a choice.
3. **`--auto` should weigh `-n` against the prompt**, once the per-machine ratio
   is measurable rather than assumed.
