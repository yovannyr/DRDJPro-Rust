//! GPL-3.0-only xwax decoder. Construction belongs off audio callbacks.
//! Only explicitly named upstream record editions are supported. Physical
//! compatibility/accuracy are separate from synthetic decoder verification.
use std::{ffi::c_void, fmt, marker::PhantomData, ptr::NonNull, sync::Mutex};

/// Maximum stereo frames submitted per worker operation.
pub const MAX_BLOCK_FRAMES: usize = 4096;
/// Beyond this age, never extrapolate an old absolute stamp into fresh control.
pub const MAX_STAMP_AGE: f64 = 0.020;
static INITIALIZE: Mutex<()> = Mutex::new(());

/// Exact upstream record definitions, not a generic Serato compatibility claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Record {
    SeratoSecondEditionA,
    SeratoSecondEditionB,
    SeratoCd,
}
impl Record {
    fn name(self) -> &'static std::ffi::CStr {
        match self {
            Self::SeratoSecondEditionA => c"serato_2a",
            Self::SeratoSecondEditionB => c"serato_2b",
            Self::SeratoCd => c"serato_cd",
        }
    }
}
/// Invalid application input or failure to construct the native decoder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidRate,
    InvalidBlock,
    Initialization,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidRate => "DVS requires a sample rate between 8 and 192 kHz",
            Self::InvalidBlock => "DVS requires a nonempty, bounded, complete stereo block",
            Self::Initialization => "xwax initialization failed",
        })
    }
}
impl std::error::Error for Error {}
/// Absolute decoded control at the end of the submitted PCM block.
#[derive(Clone, Copy, Debug)]
pub struct Observation {
    /// Record-time seconds at nominal reference speed (not application time).
    pub position_seconds: f64,
    /// Signed speed; negative means reverse. Never replaced by a simulated 1.0.
    pub speed: f64,
    pub stamp_age_seconds: f64,
}

unsafe extern "C" {
    fn drdj_xwax_new(name: *const std::ffi::c_char, rate: u32, phono: i32) -> *mut c_void;
    fn drdj_xwax_delete(ptr: *mut c_void);
    fn drdj_xwax_submit(ptr: *mut c_void, pcm: *const i16, frames: usize);
    fn drdj_xwax_position(ptr: *mut c_void, age: *mut f64) -> i32;
    fn drdj_xwax_pitch(ptr: *mut c_void) -> f64;
    fn drdj_xwax_resolution(ptr: *mut c_void) -> f64;
    fn drdj_xwax_safe(ptr: *mut c_void) -> u32;
    fn drdj_xwax_reset(ptr: *mut c_void, rate: u32, phono: i32);
}

/// Single-owner decoder, intentionally neither Send nor Sync. Construct, use and
/// destroy it on the decoding worker. Its lookup tables are initialized under
/// a process mutex and retained for process lifetime (bounded by three editions).
pub struct Decoder {
    native: NonNull<c_void>,
    rate: u32,
    phono: bool,
    _owner: PhantomData<std::rc::Rc<()>>,
}
impl Decoder {
    /// Builds the edition's shared lookup table once; may allocate/log/block.
    pub fn new(record: Record, rate: u32, phono: bool) -> Result<Self, Error> {
        if !(8000..=192000).contains(&rate) {
            return Err(Error::InvalidRate);
        }
        let _guard = INITIALIZE.lock().map_err(|_| Error::Initialization)?;
        // SAFETY: validated rate, static NUL-terminated supported edition name;
        // shared upstream initialization is serialized and tables never freed.
        let native =
            NonNull::new(unsafe { drdj_xwax_new(record.name().as_ptr(), rate, i32::from(phono)) })
                .ok_or(Error::Initialization)?;
        Ok(Self {
            native,
            rate,
            phono,
            _owner: PhantomData,
        })
    }
    /// Decode complete interleaved stereo i16 frames without Rust allocation.
    /// None means unverified, stale, outside the safe area or stationary control.
    /// Callers must release/pause DVS ownership on loss; no automatic autoplay.
    pub fn submit(&mut self, pcm: &[i16]) -> Result<Option<Observation>, Error> {
        if pcm.is_empty() || !pcm.len().is_multiple_of(2) || pcm.len() / 2 > MAX_BLOCK_FRAMES {
            return Err(Error::InvalidBlock);
        }
        let mut age = 0.0;
        // SAFETY: exclusive live opaque C object; validated complete frame count;
        // C only reads the borrowed input and retains no pointer to it.
        let (position, speed, resolution, safe) = unsafe {
            let ptr = self.native.as_ptr();
            drdj_xwax_submit(ptr, pcm.as_ptr(), pcm.len() / 2);
            (
                drdj_xwax_position(ptr, &mut age),
                drdj_xwax_pitch(ptr),
                drdj_xwax_resolution(ptr),
                drdj_xwax_safe(ptr),
            )
        };
        if position < 0
            || position as u32 >= safe
            || !speed.is_finite()
            || !age.is_finite()
            || !(0.0..=MAX_STAMP_AGE).contains(&age)
            || speed.abs() < 0.01
            || speed.abs() > 8.0
            || !resolution.is_finite()
            || resolution <= 0.0
        {
            if age > MAX_STAMP_AGE {
                self.reset();
            }
            return Ok(None);
        }
        let position_seconds = f64::from(position) / resolution + age * speed;
        if position_seconds < 0.0 {
            return Ok(None);
        }
        Ok(Some(Observation {
            position_seconds,
            speed,
            stamp_age_seconds: age,
        }))
    }
    /// Reset signal history after capture discontinuity, without rebuilding LUTs.
    pub fn reset(&mut self) {
        // SAFETY: exclusive live object; wrapper retains immutable shared edition.
        unsafe {
            drdj_xwax_reset(self.native.as_ptr(), self.rate, i32::from(self.phono));
        }
    }
}
impl Drop for Decoder {
    fn drop(&mut self) {
        // SAFETY: pointer was allocated by new, uniquely owned, freed exactly once.
        unsafe {
            drdj_xwax_delete(self.native.as_ptr());
        }
    }
}
