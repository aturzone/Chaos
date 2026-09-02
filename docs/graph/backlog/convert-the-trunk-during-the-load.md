---
topic: C7 converts the trunk after the load, which helps only a machine the trunk already fits. Converting during the load is what a smaller machine needs, and it is a different change.
status: proposed, not started
links:
  - ../research/requantising-the-trunk-2026-09-02.md
  - the-big-bang-5-tok-s.md
  - ../reference/hard-won-facts.md
---

# Convert the trunk during the load, not after it

`--trunk-quant q4_k` shipped as a **post-load** pass: `ResidentSet::load` reads
the trunk at `Q8_0`, and then `chaos_arch::requantise` takes each tensor out,
converts it, and hands it back smaller. That works, and it is measured. It also
has one structural limit, which this node exists to name rather than leave for
someone to discover.

## The limit

The load's budget is a hard ceiling applied to the **stored** size. So on a
machine where the trunk does not fit at `Q8_0`, the loader has already decided
what to skip before anything can be converted, and those tensors are recorded as
over-budget and stream from disk for the rest of the session. Converting what
*did* fit frees RAM that then cannot be spent: the expert cache is refused
outright while any of the always-read set is streaming, because a resident byte
is read every token and a cached expert byte is worth about an eighth of that.

The engine therefore refuses in that case, and says so:

```
trunk      not converted: 2.14 GiB of the always-read set is already
trunk      streaming, so a narrower trunk would lose accuracy without
trunk      buying anything. Free some RAM and it becomes worth it.
```

**Which means the lever helps exactly the machines that least need it.** On this
laptop the trunk fits at `Q8_0`, so the conversion buys expert cache. On a 12 GiB
machine — where 7.38 GiB of trunk against a 144 GB model is the difference
between streaming 3.4 GiB a token and streaming none of it — it does nothing at
all.

## What the change is

Convert **as each tensor is read**, so the budget applies to the converted size.
A 7.38 GiB trunk becomes ~4.3 GiB, and a machine with 5 GiB usable holds all of
it instead of 68% of it. That is worth far more than the cache sizing this
already buys: it removes a per-token disk read rather than caching one.

Three pieces, and the awkward one is the third:

1. **`Plan::build` has to plan in converted sizes.** It sums `loc.size` today.
   It needs a per-tensor "size if converted", which is
   `chaos_ggml::row_size(target, ne0) * nrows` — arithmetic, not a read.
2. **The loader has to convert between the read and the insert**, per tensor,
   inside the existing parallel read.
3. **`chaos-model` cannot call ggml.** Twelve of the thirteen CI-checked crates
   build with no `GGML_LIB_DIR` and CI enforces it; `chaos-model` is one of them,
   and `quantize` is ggml's. So the conversion has to arrive as a trait object or
   a pair of closures passed in by `chaos-arch`, which owns the ggml dependency.
   That is the only real design decision here, and it is the reason this was not
   done in the same change.

## The second prize, and it may be the bigger one

**Cache the converted trunk on disk.** The conversion costs ~28 s of CPU on
every load, every time, for a result that is a pure function of (container,
target type, the exclusion list). Written once beside the model as a sidecar, a
later load reads 4.3 GiB instead of 7.38 and converts nothing — so the feature
would make loading *faster* rather than 28 s slower, on a model whose load is
already the slowest thing about starting it.

What that needs to be safe: the sidecar has to name the container it came from
and be invalidated by its size and mtime, and a truncated sidecar has to be
detected rather than bound. `chaos-pull` already learned that lesson the hard
way — a resumed download came out **too large** and passed every check that was
not an exact byte count.

## Why this is not urgent

The measured win from what shipped is real but modest, and the machine that
would gain most from the during-load version is a machine nobody here has. So
this is filed with its reasoning intact rather than built on a guess about a
machine class — the same call `bigger-machine-prompt.md` makes about the
frontier numbers.
