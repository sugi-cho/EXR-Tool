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

    pub fn displays(&self) -> Vec<String> {
        let n = unsafe { ffi::ocio_config_num_displays(self.ptr) } as usize;
        (0..n)
            .filter_map(|i| unsafe {
                let p = ffi::ocio_config_get_display_name(self.ptr, i as i32);
                if p.is_null() {
                    None
                } else {
                    Some(std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned())
                }
            })
            .collect()
    }

    pub fn views(&self, display: &str) -> Vec<String> {
        let d = CString::new(display).unwrap_or_default();
        let n = unsafe { ffi::ocio_config_num_views(self.ptr, d.as_ptr()) } as usize;
        (0..n)
            .filter_map(|i| unsafe {
                let p = ffi::ocio_config_get_view_name(self.ptr, d.as_ptr(), i as i32);
                if p.is_null() {
                    None
                } else {
                    Some(std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned())
                }
            })
            .collect()
    }

    pub fn processor_display_view(&self, display: &str, view: &str) -> Result<Processor> {
        let d = CString::new(display)?;
        let v = CString::new(view)?;
        let p = unsafe { ffi::ocio_config_get_processor_display_view(self.ptr, d.as_ptr(), v.as_ptr()) };
        if p.is_null() {
            Err(anyhow!("failed to create OCIO display/view processor"))
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
