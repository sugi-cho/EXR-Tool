use std::ffi::CString;
use std::path::Path;
use anyhow::{anyhow, Result};

mod ffi {
    #![allow(non_camel_case_types, non_snake_case, non_upper_case_globals, dead_code)]
    include!(concat!(env!("OUT_DIR"), "/ocio_bindings.rs"));
}

pub struct Config {
    ptr: ffi::OcioConfig,
}

impl Config {
    pub fn from_file(path: &Path) -> Result<Self> {
        let cpath = CString::new(path.to_string_lossy().as_bytes())?;
        let ptr = unsafe { ffi::ocio_config_from_file(cpath.as_ptr()) };
        if ptr.is_null() {
            Err(anyhow!("failed to load OCIO config"))
        } else {
            Ok(Config { ptr })
        }
    }

    pub fn processor(&self, src: &str, dst: &str) -> Result<Processor> {
        let csrc = CString::new(src)?;
        let cdst = CString::new(dst)?;
        let p = unsafe { ffi::ocio_config_get_processor(self.ptr, csrc.as_ptr(), cdst.as_ptr()) };
        if p.is_null() {
            Err(anyhow!("failed to create OCIO processor"))
        } else {
            Ok(Processor { ptr: p })
        }
    }
}

impl Drop for Config {
    fn drop(&mut self) {
        unsafe { ffi::ocio_config_release(self.ptr) }
    }
}

pub struct Processor {
    ptr: ffi::OcioProcessor,
}

impl Processor {
    pub fn apply_rgb(&self, rgb: &mut [f32;3]) {
        unsafe { ffi::ocio_processor_apply_rgb(self.ptr, rgb.as_mut_ptr()) }
    }
}

impl Drop for Processor {
    fn drop(&mut self) {
        unsafe { ffi::ocio_processor_release(self.ptr) }
    }
}
