//! The arithmetic we borrow.
//!
//! Chaos's contribution is the memory side — deciding what lives in RAM,
//! streaming the rest, and scheduling reads so a model far larger than the
//! machine still runs. The arithmetic underneath (quantized matmul kernels,
//! hand-written SIMD per instruction set) is years of specialist work that is
//! already done well in `ggml`. Rewriting it would be a multi-year detour that
//! makes the product no better.
//!
//! So this crate is deliberately thin: enough FFI to turn the quantized bytes
//! our loader produces into numbers, and no more. It is not a `ggml` wrapper
//! and does not aspire to be one.
//!
//! # Building
//!
//! Set `GGML_LIB_DIR` to a directory holding `ggml-base.a`, `ggml-cpu.a` and
//! `ggml.a`. Without it the crate still compiles — every entry point returns
//! [`GgmlError::Unavailable`] — so the workspace builds on a machine that has
//! not built `ggml`.

use std::fmt;

use chaos_gguf::GgmlType;

#[cfg(have_ggml)]
pub mod backend;
pub mod device;
mod graph;
pub mod repack;
#[cfg(have_ggml)]
pub mod sched;
mod weights;

#[cfg(have_ggml)]
pub use backend::{
    download, download_f32, upload, upload_f32, Backend, Compute, DeviceBuffer, GraphAllocator,
};
// `device` is unconditional, unlike everything around it: it answers
// `Unavailable` rather than vanishing when ggml is absent, so a caller can ask
// "is there a GPU here?" in a build that cannot use one and get an answer
// instead of a missing symbol.
pub use device::{best_offload_device, devices, vulkan_available, DeviceInfo, DeviceKind};
#[cfg(have_ggml)]
pub use graph::{arena_for, f16_to_f32, f32_to_f16, Context, RopeParams, Tensor};
pub use repack::{is_repackable, Repacked};
#[cfg(have_ggml)]
pub use sched::{AlignedBytes, HostBuffer, OwnedHostBuffer, Scheduler};
#[cfg(have_ggml)]
pub use weights::{Residency, UploadReport, WeightSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GgmlError {
    /// The crate was built without linking ggml.
    Unavailable,
    /// ggml does not know this type, or cannot convert it to floats.
    UnsupportedType(u32),
    /// Element count is not a whole number of blocks.
    PartialBlock { elements: usize, block_size: i64 },
    /// The input buffer is the wrong size for the requested element count.
    WrongSize { expected: usize, actual: usize },
    /// ggml refused to create a context of this size.
    ContextAlloc { bytes: usize },
    /// The context's arena ran out while building the graph.
    ArenaExhausted,
    /// Graph execution returned a non-zero status.
    ComputeFailed(i32),
    /// No device at that index in the backend registry.
    NoSuchDevice(usize),
    /// The device exists but refused to produce a backend or a buffer type.
    DeviceInitFailed(usize),
    /// The device could not allocate a buffer for the context's tensors.
    ///
    /// Distinct from `ArenaExhausted`, which is host memory: this one means the
    /// *card* is full, and the answer is a smaller model or fewer resident
    /// layers rather than a bigger arena.
    DeviceOutOfMemory,
    /// Host memory offered to ggml as a buffer is not `TENSOR_ALIGNMENT`-aligned.
    ///
    /// **This one exists because ggml aborts instead of refusing.**
    /// `ggml_backend_cpu_buffer_from_ptr` asserts the pointer is 32-aligned and
    /// a `Vec<u8>` is aligned to 1, so the natural call takes the whole process
    /// down with `GGML_ASSERT ... "buffer pointer must be aligned"` — reported
    /// as "process didn't exit successfully", not as a failure anyone can
    /// catch. Checked on our side so it becomes a value.
    Misaligned { address: usize, required: usize },
    /// The target type cannot be produced without an importance matrix.
    ///
    /// The `IQ*` types are trained against activation statistics; `ggml`'s own
    /// `ggml_quantize_requires_imatrix` says which. Asked for one of them with
    /// no matrix, ggml quantizes to *something* and the result is far worse than
    /// the type's reputation, so it is refused by name instead.
    NeedsImatrix(u32),
}

impl fmt::Display for GgmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GgmlError::Unavailable => f.write_str(
                "built without ggml: set GGML_LIB_DIR to a directory containing \
                 ggml-base.a, ggml-cpu.a and ggml.a, then rebuild",
            ),
            GgmlError::UnsupportedType(t) => {
                write!(f, "ggml cannot convert type {t} to floats")
            }
            GgmlError::PartialBlock {
                elements,
                block_size,
            } => write!(
                f,
                "{elements} elements is not a whole number of {block_size}-element blocks"
            ),
            GgmlError::WrongSize { expected, actual } => {
                write!(f, "buffer is {actual} bytes, expected {expected}")
            }
            GgmlError::ContextAlloc { bytes } => {
                write!(f, "ggml refused a context arena of {bytes} bytes")
            }
            GgmlError::ArenaExhausted => f.write_str(
                "the ggml arena ran out while building the graph; give the context more memory",
            ),
            GgmlError::ComputeFailed(s) => {
                write!(f, "ggml graph computation failed with status {s}")
            }
            GgmlError::NoSuchDevice(i) => {
                write!(f, "no compute device at index {i}")
            }
            GgmlError::DeviceInitFailed(i) => {
                write!(f, "device {i} refused to initialise a backend")
            }
            GgmlError::DeviceOutOfMemory => f.write_str(
                "the device could not allocate the requested tensors; it is out of memory, \
                 which needs a smaller model rather than a bigger arena",
            ),
            GgmlError::NeedsImatrix(t) => write!(
                f,
                "type {t} needs an importance matrix, which this build does not compute;                  pick a K-quant instead"
            ),
            GgmlError::Misaligned { address, required } => write!(
                f,
                "host memory at {address:#x} is {} bytes off a {required}-byte boundary; \
                 ggml requires buffer pointers and tensor offsets to be {required}-aligned",
                address % required
            ),
        }
    }
}

impl std::error::Error for GgmlError {}

pub type Result<T> = std::result::Result<T, GgmlError>;

/// True when this build can actually call `ggml`.
pub const fn available() -> bool {
    cfg!(have_ggml)
}

/// `GGML_TYPE_F32`. Special-cased because ggml offers no conversion kernel
/// for it — the conversion is the identity.
// Referenced only by the ggml-backed paths, so a build without ggml sees it
// as dead. It is the type tag, not a convenience constant -- keep it.
#[cfg_attr(not(have_ggml), allow(dead_code))]
const GGML_TYPE_F32: u32 = 0;

#[cfg(have_ggml)]
mod ffi {
    use std::os::raw::{c_char, c_int, c_void};

    pub type ToFloat = unsafe extern "C" fn(*const c_void, *mut f32, i64);
    pub type FromFloatRef = unsafe extern "C" fn(*const f32, *mut c_void, i64);

    #[repr(C)]
    pub struct TypeTraits {
        pub type_name: *const c_char,
        pub blck_size: i64,
        pub blck_size_interleave: i64,
        pub type_size: usize,
        pub is_quantized: bool,
        pub to_float: Option<ToFloat>,
        pub from_float_ref: Option<FromFloatRef>,
    }

    // Declared as a set even though only `ggml_get_type_traits` is called
    // today: getting an FFI signature wrong is silent corruption, so these are
    // transcribed once, together, from one header revision rather than added
    // piecemeal later under time pressure.
    #[allow(dead_code)]
    extern "C" {
        pub fn ggml_get_type_traits(ty: c_int) -> *const TypeTraits;
        pub fn ggml_type_size(ty: c_int) -> usize;
        pub fn ggml_blck_size(ty: c_int) -> i64;
        pub fn ggml_type_name(ty: c_int) -> *const c_char;
        pub fn ggml_row_size(ty: c_int, ne: i64) -> usize;
        pub fn ggml_quantize_requires_imatrix(ty: c_int) -> bool;
        pub fn ggml_quantize_chunk(
            ty: c_int,
            src: *const f32,
            dst: *mut c_void,
            start: i64,
            nrows: i64,
            n_per_row: i64,
            imatrix: *const f32,
        ) -> usize;
    }
}

/// What `ggml` reports about a tensor type.
///
/// Worth cross-checking against our own table in `chaos-gguf`: if the two
/// disagree about a block size, one of them is wrong and every byte count
/// derived from it is too.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeInfo {
    pub name: String,
    pub block_elems: i64,
    pub block_bytes: usize,
    pub is_quantized: bool,
    pub can_dequantize: bool,
}

/// Ask `ggml` about a type.
pub fn type_info(ty: GgmlType) -> Result<TypeInfo> {
    #[cfg(not(have_ggml))]
    {
        let _ = ty;
        Err(GgmlError::Unavailable)
    }
    #[cfg(have_ggml)]
    {
        // SAFETY: ggml_get_type_traits is a pure lookup over a static table.
        // It returns a pointer into that table, valid for the process lifetime.
        let traits = unsafe { ffi::ggml_get_type_traits(ty.0 as i32) };
        if traits.is_null() {
            return Err(GgmlError::UnsupportedType(ty.0));
        }
        // SAFETY: non-null pointer into ggml's static type table.
        let t = unsafe { &*traits };
        // SAFETY: type_name is a static NUL-terminated C string.
        let name = unsafe { std::ffi::CStr::from_ptr(t.type_name) }
            .to_string_lossy()
            .into_owned();
        Ok(TypeInfo {
            name,
            block_elems: t.blck_size,
            block_bytes: t.type_size,
            is_quantized: t.is_quantized,
            can_dequantize: t.to_float.is_some(),
        })
    }
}

/// Convert quantized bytes to `f32`, using `ggml`'s kernel for the type.
///
/// This is the join between our loader and the math: `data` is exactly what
/// [`chaos_model::Model::read_tensor`] returns, still in its stored format.
pub fn dequantize(ty: GgmlType, data: &[u8], elements: usize) -> Result<Vec<f32>> {
    #[cfg(not(have_ggml))]
    {
        let _ = (ty, data, elements);
        Err(GgmlError::Unavailable)
    }
    #[cfg(have_ggml)]
    {
        // SAFETY: pure lookup into ggml's static type table.
        let traits = unsafe { ffi::ggml_get_type_traits(ty.0 as i32) };
        if traits.is_null() {
            return Err(GgmlError::UnsupportedType(ty.0));
        }
        // SAFETY: non-null pointer into a static table.
        let t = unsafe { &*traits };

        if t.blck_size <= 0 || elements as i64 % t.blck_size != 0 {
            return Err(GgmlError::PartialBlock {
                elements,
                block_size: t.blck_size,
            });
        }
        // The kernel reads exactly this many bytes; a short buffer would read
        // out of bounds, so it is checked rather than trusted. Checked before
        // the F32 path too, so both routes reject a malformed buffer alike.
        let expected = elements / t.blck_size as usize * t.type_size;
        if data.len() != expected {
            return Err(GgmlError::WrongSize {
                expected,
                actual: data.len(),
            });
        }

        // ggml supplies no `to_float` for F32 because the conversion is a
        // no-op -- there is genuinely nothing to call. Reinterpret instead of
        // reporting the type as unsupported.
        if ty.0 == GGML_TYPE_F32 {
            return Ok(data
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect());
        }

        let Some(to_float) = t.to_float else {
            return Err(GgmlError::UnsupportedType(ty.0));
        };

        let mut out = vec![0f32; elements];
        // SAFETY: `data` holds exactly `expected` bytes as checked above, which
        // is what the kernel reads for `elements` values; `out` has capacity for
        // `elements` floats, which is what it writes. Neither aliases the other.
        unsafe {
            to_float(
                data.as_ptr() as *const std::os::raw::c_void,
                out.as_mut_ptr(),
                elements as i64,
            );
        }
        Ok(out)
    }
}

/// Bytes one row of `ne` values occupies in `ty`, as **ggml** computes it.
///
/// Not derived from our own block table: this is the number the kernel will
/// write, and a quantizer that sizes its destination from a second opinion is
/// one table revision away from a heap overflow.
pub fn row_size(ty: GgmlType, ne: i64) -> Result<usize> {
    #[cfg(not(have_ggml))]
    {
        let _ = (ty, ne);
        Err(GgmlError::Unavailable)
    }
    #[cfg(have_ggml)]
    {
        // SAFETY: pure lookup into ggml's static type table.
        let traits = unsafe { ffi::ggml_get_type_traits(ty.0 as i32) };
        if traits.is_null() {
            return Err(GgmlError::UnsupportedType(ty.0));
        }
        // SAFETY: non-null pointer into a static table.
        let block = unsafe { &*traits }.blck_size;
        if block <= 0 || ne % block != 0 {
            return Err(GgmlError::PartialBlock {
                elements: ne.max(0) as usize,
                block_size: block,
            });
        }
        // SAFETY: the type is known to ggml and the row is a whole number of
        // blocks, which is the only precondition `ggml_row_size` asserts.
        Ok(unsafe { ffi::ggml_row_size(ty.0 as i32, ne) })
    }
}

/// Quantize `nrows` rows of `n_per_row` floats into `ty`, writing into `dst`.
///
/// The inverse of [`dequantize`], and the half this crate did not have. It
/// exists for **C7**: V4-Flash's always-read trunk is stored `Q8_0` at 1.06
/// bytes a weight while its routed experts are `MXFP4` at 0.53, so the set that
/// has to stay in RAM forever is the one stored at twice the width. Moving it to
/// a K-quant at load is the only lever left that changes how much of a 15.7 GiB
/// machine is free for anything else.
///
/// # Rows, not tensors
///
/// `ggml_quantize_chunk` works in whole rows because that is the unit a scale
/// covers — K-quants search for scales within a 256-value super-block and never
/// across a row boundary. Taking rows rather than a tensor is what lets a caller
/// convert 7 GiB in bounded slices instead of holding the whole thing as `f32`
/// first, which on this machine would not fit.
///
/// # Errors
///
/// Refuses rather than guesses: a partial block, a `src` that does not match
/// `nrows * n_per_row`, a `dst` too small for what the kernel will write, and
/// the `IQ*` types, which need an importance matrix this build does not compute
/// and would otherwise be quantized badly and silently.
pub fn quantize(
    ty: GgmlType,
    src: &[f32],
    nrows: i64,
    n_per_row: i64,
    dst: &mut [u8],
) -> Result<usize> {
    #[cfg(not(have_ggml))]
    {
        let _ = (ty, src, nrows, n_per_row, dst);
        Err(GgmlError::Unavailable)
    }
    #[cfg(have_ggml)]
    {
        if nrows <= 0 || n_per_row <= 0 {
            return Err(GgmlError::PartialBlock {
                elements: 0,
                block_size: n_per_row,
            });
        }
        let elements = (nrows as usize).saturating_mul(n_per_row as usize);
        if src.len() != elements {
            return Err(GgmlError::WrongSize {
                expected: elements,
                actual: src.len(),
            });
        }
        // SAFETY: pure lookup into ggml's static type table.
        if unsafe { ffi::ggml_quantize_requires_imatrix(ty.0 as i32) } {
            return Err(GgmlError::NeedsImatrix(ty.0));
        }
        let row = row_size(ty, n_per_row)?;
        let expected = row.saturating_mul(nrows as usize);
        if dst.len() < expected {
            return Err(GgmlError::WrongSize {
                expected,
                actual: dst.len(),
            });
        }

        // SAFETY: `src` holds exactly `nrows * n_per_row` floats, which with
        // `start = 0` is the range the kernel reads; `dst` holds at least
        // `nrows * row_size` bytes, which is what it writes, and ggml computed
        // that row size itself. The type is not an `IQ*`, so the null imatrix is
        // the documented "none" rather than a missing argument.
        let written = unsafe {
            ffi::ggml_quantize_chunk(
                ty.0 as i32,
                src.as_ptr(),
                dst.as_mut_ptr() as *mut std::os::raw::c_void,
                0,
                nrows,
                n_per_row,
                std::ptr::null(),
            )
        };
        if written != expected {
            // ggml returns what it wrote. A disagreement means our row size and
            // its kernel do not match, and every byte after the first row would
            // be at the wrong offset -- reported rather than bound.
            return Err(GgmlError::WrongSize {
                expected,
                actual: written,
            });
        }
        Ok(written)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_whether_ggml_is_linked() {
        // Not an assertion about which: the crate must build both ways.
        let linked = available();
        if !linked {
            assert_eq!(type_info(GgmlType(0)), Err(GgmlError::Unavailable));
        }
    }

    #[cfg(have_ggml)]
    #[test]
    fn ggml_agrees_with_our_own_block_size_table() {
        // If these disagree, one table is wrong and every byte count derived
        // from it is wrong too -- including the numbers the whole plan rests on.
        for id in [0u32, 1, 8, 12, 14, 19, 23, 29, 30] {
            let ours = GgmlType(id);
            let (Some(our_elems), Some(our_bytes)) = (ours.block_elems(), ours.block_bytes())
            else {
                continue;
            };
            let theirs = type_info(ours).expect("ggml knows this type");
            assert_eq!(
                our_elems as i64, theirs.block_elems,
                "block elems disagree for type {id} ({})",
                theirs.name
            );
            assert_eq!(
                our_bytes as usize, theirs.block_bytes,
                "block bytes disagree for type {id} ({})",
                theirs.name
            );
        }
    }

    #[cfg(have_ggml)]
    #[test]
    fn dequantizes_f32_as_an_identity() {
        // F32 -> f32 must be exact; anything else means the plumbing is wrong.
        let values: Vec<f32> = (0..64).map(|i| i as f32 * 0.5 - 8.0).collect();
        let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        let out = dequantize(GgmlType(0), &bytes, values.len()).expect("dequantize");
        assert_eq!(out, values);
    }

    #[cfg(have_ggml)]
    #[test]
    fn rejects_a_buffer_of_the_wrong_size() {
        // Trusting the caller here would read out of bounds.
        let err = dequantize(GgmlType(0), &[0u8; 8], 64);
        assert!(matches!(err, Err(GgmlError::WrongSize { .. })));
    }

    #[cfg(have_ggml)]
    #[test]
    fn rejects_a_partial_block() {
        // Q4_K packs 256 elements per block; 100 is not a whole number of them.
        let err = dequantize(GgmlType(12), &[0u8; 144], 100);
        assert!(matches!(err, Err(GgmlError::PartialBlock { .. })));
    }

    /// A smooth signal, which is what a weight row resembles more than noise
    /// does — and unlike noise it makes a quantizer's error interpretable.
    #[cfg(have_ggml)]
    fn signal(n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| {
                let x = i as f32 * 0.017;
                x.sin() * 0.35 + (x * 0.31).cos() * 0.12
            })
            .collect()
    }

    #[cfg(have_ggml)]
    #[test]
    fn round_trips_through_q8_0_and_q4_k() {
        // The measurement C7 rests on: how much accuracy a trunk row loses when
        // it moves from Q8_0 to Q4_K. Asserted loosely, and printed exactly.
        const NE: i64 = 4096;
        const ROWS: i64 = 4;
        let src = signal((NE * ROWS) as usize);

        for (ty, name, bound) in [
            (GgmlType(8), "Q8_0", 0.004f32),
            (GgmlType(12), "Q4_K", 0.02),
        ] {
            let row = row_size(ty, NE).expect("row size");
            let mut dst = vec![0u8; row * ROWS as usize];
            let written = quantize(ty, &src, ROWS, NE, &mut dst).expect("quantize");
            assert_eq!(written, dst.len(), "{name} wrote a different byte count");

            let back = dequantize(ty, &dst, src.len()).expect("dequantize");
            let rms = (src
                .iter()
                .zip(&back)
                .map(|(a, b)| ((a - b) as f64).powi(2))
                .sum::<f64>()
                / src.len() as f64)
                .sqrt() as f32;
            println!(
                "{name}: {} bytes/weight, rms {rms:.6}",
                written as f32 / src.len() as f32
            );
            assert!(rms < bound, "{name} rms {rms} exceeds {bound}");
        }
    }

    #[cfg(have_ggml)]
    #[test]
    fn quantize_refuses_what_it_cannot_do_correctly() {
        let src = signal(512);
        let mut dst = vec![0u8; 4096];

        // 300 is not a whole number of 256-element Q4_K blocks.
        assert!(matches!(
            quantize(GgmlType(12), &src[..300], 1, 300, &mut dst),
            Err(GgmlError::PartialBlock { .. })
        ));
        // A src that does not match nrows * n_per_row.
        assert!(matches!(
            quantize(GgmlType(12), &src, 4, 256, &mut dst),
            Err(GgmlError::WrongSize { .. })
        ));
        // A dst too small for what the kernel would write.
        assert!(matches!(
            quantize(GgmlType(12), &src, 2, 256, &mut dst[..8]),
            Err(GgmlError::WrongSize { .. })
        ));
        // IQ2_XXS is trained against activation statistics; without them ggml
        // still produces bytes, and they are much worse than the type implies.
        assert!(
            matches!(
                quantize(GgmlType(16), &src, 2, 256, &mut dst),
                Err(GgmlError::NeedsImatrix(16))
            ),
            "ggml_quantize_requires_imatrix no longer names IQ2_XXS -- \
             re-check which types need one before trusting this refusal"
        );
    }

    #[cfg(have_ggml)]
    #[test]
    fn row_size_agrees_with_our_own_table() {
        // Two tables that disagree would put every row after the first at the
        // wrong offset -- fluent nonsense, not a crash.
        for id in [0u32, 1, 8, 12, 13, 14, 30, 39] {
            let ty = GgmlType(id);
            let (Some(elems), Some(bytes)) = (ty.block_elems(), ty.block_bytes()) else {
                continue;
            };
            let ne = (elems * 4) as i64;
            let ours = (bytes * 4) as usize;
            assert_eq!(
                row_size(ty, ne).expect("row size"),
                ours,
                "row size disagrees for type {id}"
            );
        }
    }

    #[cfg(have_ggml)]
    #[test]
    fn row_size_refuses_a_partial_row() {
        assert!(matches!(
            row_size(GgmlType(12), 100),
            Err(GgmlError::PartialBlock { .. })
        ));
    }
}
