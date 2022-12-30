pub extern crate paste;
#[macro_export]
macro_rules! export_mb_backdoor_dpi {
    ($mailbox:ident) => {
        export_mb_backdoor_dpi!(@ u8, $mailbox);
        export_mb_backdoor_dpi!(@ u16, $mailbox);
        export_mb_backdoor_dpi!(@ u32, $mailbox);
        export_mb_backdoor_dpi!(@ u64, $mailbox);
        #[no_mangle]
        extern "C" fn mb_get_space(
            ch_name: *const std::os::raw::c_char,
            space_name: *mut *const std::os::raw::c_char,
        ) {
            let name = unsafe { std::ffi::CStr::from_ptr(ch_name) }.to_str().unwrap();
            let space_name_str = $mailbox
                .get_ch_space_name(name)
                .expect(format!("can't find space of ch {}!", name).as_str());
            let c_str = std::ffi::CString::new(space_name_str).unwrap();
            let ptr = c_str.as_ptr();
            std::mem::forget(c_str);
            unsafe {
                *space_name = ptr;
            }
        }

        #[no_mangle]
        extern "C" fn mb_backdoor_write_string(
            space_name: *const std::os::raw::c_char,
            addr: u64,
            data: *const std::os::raw::c_char,
        ) {
            let space_name = unsafe { std::ffi::CStr::from_ptr(space_name) }.to_str().unwrap();
            let mut space = $mailbox
                .get_space(space_name)
                .expect(format!("space {} does not exist!", space_name).as_str())
                .lock()
                .unwrap();
            let m_data = unsafe { std::ffi::CStr::from_ptr(data) }.to_bytes_with_nul();
            space.write(addr as crate::mailbox_rs::mb_rpcs::MBPtrT, m_data);
        }

        #[no_mangle]
        extern "C" fn mb_backdoor_read_string(
            space_name: *const std::os::raw::c_char,
            addr: u64,
            data: *mut *const std::os::raw::c_char,
        ) {
            let space_name = unsafe { std::ffi::CStr::from_ptr(space_name) }.to_str().unwrap();
            let space = $mailbox
                .get_space(space_name)
                .expect(format!("space {} does not exist!", space_name).as_str());
            let resolver = crate::mailbox_rs::mb_std::MBSMPtrResolver::new(space);
            let s = resolver.read_c_str(addr as *const u8).unwrap();
            let c_str = std::ffi::CString::new(s.as_str()).unwrap();
            let ptr = c_str.as_ptr();
            std::mem::forget(c_str);
            unsafe {
                *data = ptr;
            }
        }
    };
    (@ $t:ty, $mailbox:ident) => {
        crate::mailbox_rs::mb_std::paste::paste!{
            #[no_mangle]
            extern "C" fn [<mb_backdoor_write_ $t>](space_name: *const std::os::raw::c_char, addr: u64, data: $t) {
                let space_name = unsafe { std::ffi::CStr::from_ptr(space_name) }.to_str().unwrap();
                let mut space = $mailbox
                    .get_space(space_name)
                    .expect(format!("space {} does not exist!", space_name).as_str())
                    .lock()
                    .unwrap();
                space.write_sized(addr as crate::mailbox_rs::mb_rpcs::MBPtrT, &data);
            }
            #[no_mangle]
            extern "C" fn [<mb_backdoor_read_ $t>](
                space_name: *const std::os::raw::c_char,
                addr: u64,
                data: *mut $t,
            ) {
                let space_name = unsafe { std::ffi::CStr::from_ptr(space_name) }.to_str().unwrap();
                let space = $mailbox
                    .get_space(space_name)
                    .expect(format!("space {} does not exist!", space_name).as_str())
                    .lock()
                    .unwrap();
                let m_data = unsafe { std::slice::from_raw_parts_mut(data as *mut u8, std::mem::size_of::<$t>()) };
                space.read(addr as crate::mailbox_rs::mb_rpcs::MBPtrT, m_data);
            }
        }

    };
}

#[macro_export]
macro_rules! export_mb_backdoor_py {
    ($mailbox:ident, $package:ident, $($funcs:ident),+) => {
        export_mb_backdoor_py!(@common_funcs $mailbox);
        #[pymodule]
        fn $package(_py: Python, m: &PyModule) -> PyResult<()> {
            py_mb_backdoor_add_commom_funcs(m)?;
            $(m.add_function(crate::pyo3::wrap_pyfunction!($funcs,m)?)?;)+
            Ok(())
        }
    };
    ($mailbox:ident, $package:ident) => {
        export_mb_backdoor_py!(@common_funcs $mailbox);
        #[pymodule]
        fn $package(_py: Python, m: &PyModule) -> PyResult<()> {
            py_mb_backdoor_add_commom_funcs(m)
        }
    };
    (@common_funcs $mailbox:ident) => {
        use crate::pyo3::prelude::*;
        export_mb_backdoor_py!(@ u8, $mailbox);
        export_mb_backdoor_py!(@ u16, $mailbox);
        export_mb_backdoor_py!(@ u32, $mailbox);
        export_mb_backdoor_py!(@ u64, $mailbox);
        #[pyfunction]
        fn py_mb_get_space(
            ch_name: &str,
        ) -> String{
            let space_name_str = $mailbox
            .get_ch_space_name(ch_name)
            .expect(format!("can't find space of ch {}!", ch_name).as_str());
            space_name_str.to_string()
        }

        #[pyfunction]
        fn py_mb_backdoor_read_string(
            space_name: &str,
            addr: u64,
        ) -> String{
            let space = $mailbox
                .get_space(space_name)
                .expect(format!("space {} does not exist!", space_name).as_str());
            let resolver = crate::mailbox_rs::mb_std::MBSMPtrResolver::new(space);
            let s = resolver.read_c_str(addr as *const u8).unwrap();
            s.to_string()
        }
        #[pyfunction]
        fn py_mb_backdoor_write_string(
            space_name: &str,
            addr: u64,
            data: &str,
        ) {
            let mut space = $mailbox
                .get_space(space_name)
                .expect(format!("space {} does not exist!", space_name).as_str())
                .lock()
                .unwrap();
            let m_data = std::ffi::CString::new(data).unwrap();
            space.write(addr as crate::mailbox_rs::mb_rpcs::MBPtrT, m_data.to_bytes_with_nul());
        }
        fn py_mb_backdoor_add_commom_funcs(m: &PyModule) -> PyResult<()> {
            m.add_function(crate::pyo3::wrap_pyfunction!(py_mb_get_space, m)?)?;
            m.add_function(crate::pyo3::wrap_pyfunction!(py_mb_backdoor_read_string, m)?)?;
            m.add_function(crate::pyo3::wrap_pyfunction!(py_mb_backdoor_write_string, m)?)?;
            m.add_function(crate::pyo3::wrap_pyfunction!(py_mb_backdoor_read_u8, m)?)?;
            m.add_function(crate::pyo3::wrap_pyfunction!(py_mb_backdoor_write_u8, m)?)?;
            m.add_function(crate::pyo3::wrap_pyfunction!(py_mb_backdoor_read_u16, m)?)?;
            m.add_function(crate::pyo3::wrap_pyfunction!(py_mb_backdoor_write_u16, m)?)?;
            m.add_function(crate::pyo3::wrap_pyfunction!(py_mb_backdoor_read_u32, m)?)?;
            m.add_function(crate::pyo3::wrap_pyfunction!(py_mb_backdoor_write_u32, m)?)?;
            m.add_function(crate::pyo3::wrap_pyfunction!(py_mb_backdoor_read_u64, m)?)?;
            m.add_function(crate::pyo3::wrap_pyfunction!(py_mb_backdoor_write_u64, m)?)?;
            Ok(())
        }
    };
    (@ $t:ty, $mailbox:ident) => {
        crate::mailbox_rs::mb_std::paste::paste!{
            #[pyfunction]
            fn [<py_mb_backdoor_write_ $t>](space_name: &str, addr: u64, data: $t) {
                let mut space = $mailbox
                .get_space(space_name)
                .expect(format!("space {} does not exist!", space_name).as_str())
                .lock()
                .unwrap();
                space.write_sized(addr as crate::mailbox_rs::mb_rpcs::MBPtrT, &data);
            }
            #[pyfunction]
            fn [<py_mb_backdoor_read_ $t>](
                space_name: &str,
                addr: u64,
            ) -> $t {
                let space = $mailbox
                .get_space(space_name)
                .expect(format!("space {} does not exist!", space_name).as_str())
                .lock()
                .unwrap();
                let mut data:$t = 0;
                space.read_sized(addr as crate::mailbox_rs::mb_rpcs::MBPtrT, &mut data);
                data
            }
        }
    };
}
