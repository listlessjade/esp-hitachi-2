use std::{
    ffi::{c_char, c_int, c_void, VaListImpl},
    ops::DerefMut,
    sync::OnceLock,
};

use esp_idf_sys::{esp_log_set_vprintf, vprintf_like_t};
use thingbuf::{
    mpsc::blocking::{StaticChannel, StaticReceiver, StaticSender},
    recycling::WithCapacity,
};

static LOG_QUEUE: StaticChannel<String, 32, WithCapacity> =
    StaticChannel::with_recycle(WithCapacity::new().with_max_capacity(64));
static ORIGINAL_VPRINTF: OnceLock<vprintf_like_t> = OnceLock::new(); // oncelock sounds like an onceller x sherlock ship
static SENDER: OnceLock<StaticSender<String, WithCapacity>> = OnceLock::new();

#[no_mangle]
unsafe extern "C" fn c_library_print(str: *const c_char, args: *mut c_void) -> c_int {
    let mut va_list_inner: VaListImpl = unsafe { std::mem::transmute(args) }; // THIS IS SO SAFE
    let va_list = va_list_inner.as_va_list();

    if let Some(Some(printf)) = ORIGINAL_VPRINTF.get() {
        va_list.with_copy(|list| printf(str, unsafe { std::mem::transmute(list) }));
    }

    if let Some(mut slot) = SENDER.get().and_then(|v| v.try_send_ref().ok()) {
        let res = printf_compat::format(
            str,
            va_list,
            printf_compat::output::fmt_write(slot.deref_mut()),
        );
        slot.push('\n');
        slot.push('C'); //for c... get it... that's the letter this is nothing
        res
    } else {
        -1
    }
}

pub fn redirect_logs() -> (
    StaticSender<String, WithCapacity>,
    StaticReceiver<String, WithCapacity>,
) {
    let original_printf = unsafe { esp_log_set_vprintf(Some(c_library_print)) };
    let (tx, rx) = LOG_QUEUE.split();
    ORIGINAL_VPRINTF.set(original_printf);
    SENDER.set(tx.clone());

    (tx, rx)
}

/*
the rust log -> esp-idf-logging zone

*/

pub mod log_crate_shenanigans {

    use std::{
        collections::BTreeMap,
        ffi::{CStr, CString},
        sync::OnceLock,
    };

    use ::log::{Level, LevelFilter, Metadata, Record};
    use esp_idf_svc::private::{common::Newtype, cstr::to_cstring_arg, mutex::*};
    use esp_idf_sys::*;
    use std::fmt::Write;
    use thingbuf::{mpsc::blocking::StaticSender, recycling::WithCapacity};

    use super::SENDER;

    pub struct EspStdout(*mut FILE);

    impl Default for EspStdout {
        fn default() -> Self {
            Self::new()
        }
    }

    impl EspStdout {
        pub fn new() -> Self {
            let stdout = unsafe { __getreent().as_mut() }.unwrap()._stdout;

            let file = unsafe { stdout.as_mut() }.unwrap();

            // Copied from here:
            // https://github.com/bminor/newlib/blob/master/newlib/libc/stdio/local.h#L80
            // https://github.com/bminor/newlib/blob/3bafe2fae7a0878598a82777c623edb2faa70b74/newlib/libc/include/sys/stdio.h#L13
            if (file._flags2 & __SNLK as i32) == 0 && (file._flags & __SSTR as i16) == 0 {
                unsafe {
                    _lock_acquire_recursive(&mut file._lock);
                }
            }

            Self(stdout)
        }
    }

    impl Drop for EspStdout {
        fn drop(&mut self) {
            let file = unsafe { self.0.as_mut() }.unwrap();

            // Copied from here:
            // https://github.com/bminor/newlib/blob/master/newlib/libc/stdio/local.h#L85
            // https://github.com/bminor/newlib/blob/3bafe2fae7a0878598a82777c623edb2faa70b74/newlib/libc/include/sys/stdio.h#L21
            if (file._flags2 & __SNLK as i32) == 0 && (file._flags & __SSTR as i16) == 0 {
                unsafe {
                    _lock_release_recursive(&mut file._lock);
                }
            }
        }
    }

    impl core::fmt::Write for EspStdout {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            let slice = s.as_bytes();
            unsafe {
                fwrite(slice.as_ptr() as *const _, 1, slice.len() as u32, self.0);
            }

            Ok(())
        }
    }

    static LOGGER: EspChannelLogger = EspChannelLogger::new();
    pub static LOG_TX: OnceLock<StaticSender<String, WithCapacity>> = OnceLock::new();

    pub struct EspChannelLogger {
        // esp-idf function `esp_log_level_get` builds a cache using the address
        // of the target and not doing a string compare. This means we need to
        // build a cache of our own mapping the str value to a consistant
        // Cstr value.
        cache: Mutex<BTreeMap<String, CString>>,
    }

    unsafe impl Send for EspChannelLogger {}
    unsafe impl Sync for EspChannelLogger {}

    impl EspChannelLogger {
        /// Public in case user code would like to compose this logger in their own one
        pub const fn new() -> Self {
            Self {
                cache: Mutex::new(BTreeMap::new()),
            }
        }

        pub fn initialize_with_channel(sender_tx: StaticSender<String, WithCapacity>) {
            ::log::set_logger(&LOGGER)
                .map(|()| LOGGER.initialize())
                .map(|()| LOG_TX.set(sender_tx))
                .unwrap();
        }

        pub fn initialize(&self) {
            ::log::set_max_level(self.get_max_level());
        }

        pub fn get_max_level(&self) -> LevelFilter {
            LevelFilter::from(Newtype(CONFIG_LOG_MAXIMUM_LEVEL))
        }

        pub fn set_target_level(
            &self,
            target: impl AsRef<str>,
            level_filter: LevelFilter,
        ) -> Result<(), EspError> {
            let target = target.as_ref();

            let mut cache = self.cache.lock();

            let ctarget = loop {
                if let Some(ctarget) = cache.get(target) {
                    break ctarget;
                }

                let ctarget = to_cstring_arg(target)?;

                cache.insert(target.into(), ctarget);
            };

            unsafe {
                esp_log_level_set(
                    ctarget.as_c_str().as_ptr(),
                    Newtype::<esp_log_level_t>::from(level_filter).0,
                );
            }

            Ok(())
        }

        fn get_marker(level: Level) -> &'static str {
            match level {
                Level::Error => "E",
                Level::Warn => "W",
                Level::Info => "I",
                Level::Debug => "D",
                Level::Trace => "V",
            }
        }

        fn get_color(_level: Level) -> Option<u8> {
            #[cfg(esp_idf_log_colors)]
            {
                match _level {
                    Level::Error => Some(31), // LOG_COLOR_RED
                    Level::Warn => Some(33),  // LOG_COLOR_BROWN
                    Level::Info => Some(32),  // LOG_COLOR_GREEN,
                    _ => None,
                }
            }

            #[cfg(not(esp_idf_log_colors))]
            {
                None
            }
        }

        fn should_log(&self, record: &Record) -> bool {
            let level = Newtype::<esp_log_level_t>::from(record.level()).0;

            let mut cache = self.cache.lock();

            let ctarget = loop {
                if let Some(ctarget) = cache.get(record.target()) {
                    break ctarget;
                }

                if let Ok(ctarget) = to_cstring_arg(record.target()) {
                    cache.insert(record.target().into(), ctarget);
                } else {
                    return true;
                }
            };

            let max_level = unsafe { esp_log_level_get(ctarget.as_c_str().as_ptr()) };
            level <= max_level
        }
    }

    impl Default for EspChannelLogger {
        fn default() -> Self {
            Self::new()
        }
    }

    impl ::log::Log for EspChannelLogger {
        fn enabled(&self, metadata: &Metadata) -> bool {
            metadata.level() <= LevelFilter::from(Newtype(CONFIG_LOG_MAXIMUM_LEVEL))
        }

        fn log(&self, record: &Record) {
            let metadata = record.metadata();

            if self.enabled(metadata) && self.should_log(record) {
                let Some(mut slot) = SENDER.get().and_then(|v| v.try_send_ref().ok()) else {
                    return;
                };

                let marker = Self::get_marker(metadata.level());
                let target = record.metadata().target();
                let args = record.args();
                let color = Self::get_color(record.level());

                // let mut stdout = EspStdout::new();

                if let Some(color) = color {
                    write!(slot, "\x1b[0;{}m", color).unwrap();
                }
                write!(slot, "{} (", marker).unwrap();
                if cfg!(esp_idf_log_timestamp_source_rtos) {
                    let timestamp = unsafe { esp_log_timestamp() };
                    write!(slot, "{}", timestamp).unwrap();
                } else if cfg!(esp_idf_log_timestamp_source_system) {
                    // TODO: https://github.com/esp-rs/esp-idf-svc/pull/494 - official usage of
                    // `esp_log_timestamp_str()` should be tracked and replace the not thread-safe
                    // `esp_log_system_timestamp()` which has a race condition flaw due to
                    // returning a pointer to a static buffer containing the c-string.
                    let timestamp =
                        unsafe { CStr::from_ptr(esp_log_system_timestamp()).to_str().unwrap() };
                    write!(slot, "{}", timestamp).unwrap();
                }
                write!(slot, ") {}: {}", target, args).unwrap();
                if color.is_some() {
                    write!(slot, "\x1b[0m").unwrap();
                }
                writeln!(slot).unwrap();

                slot.push('R'); // for rust!
            }
        }

        fn flush(&self) {}
    }

    pub fn set_target_level(
        target: impl AsRef<str>,
        level_filter: LevelFilter,
    ) -> Result<(), EspError> {
        LOGGER.set_target_level(target, level_filter)
    }

    // use std::ffi::VaList;
}
