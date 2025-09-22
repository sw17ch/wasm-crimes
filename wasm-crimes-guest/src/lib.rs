pub mod wasm_crimes {
    #[link(wasm_import_module = "wasm_crimes")]
    unsafe extern "C" {
        #[link_name = "get"]
        pub unsafe fn get() -> u32;

        #[link_name = "put"]
        pub unsafe fn put(val: u32) -> u32;
    }
}

/// # Safety
///
/// This is a crime. Don't do this.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn enter_mut(ptr: u32, len: u32) -> u32 {
    // just to show the host calls work
    let v = unsafe { wasm_crimes::get() };
    unsafe { wasm_crimes::put(v.wrapping_add(1)) };

    // debug print the buffer passed
    let ptr = ptr as *mut u8;
    let len = len as usize;

    let buf = unsafe { std::slice::from_raw_parts_mut(ptr, len) };
    let _r = dbg!(std::str::from_utf8(buf));

    len as u32
}

#[cfg(target_arch = "wasm32")]
mod alloc_interface {
    use std::alloc::{Layout, alloc, dealloc, realloc};

    const _: () = assert!(std::mem::size_of::<*mut u8>() == std::mem::size_of::<u32>());
    const _: () = assert!(std::mem::size_of::<usize>() == std::mem::size_of::<u32>());

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn guest_alloc(size: u32, align: u32) -> u32 {
        let size = size as usize;
        let align = align as usize;
        let Ok(layout) = Layout::from_size_align(size, align) else {
            return 0;
        };
        let ptr = unsafe { alloc(layout) };

        ptr.addr() as u32
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn guest_dealloc(ptr: u32, size: u32, align: u32) {
        if ptr == 0 {
            return;
        }

        let ptr = ptr as *mut u8;
        let size = size as usize;
        let align = align as usize;

        let Ok(layout) = Layout::from_size_align(size, align) else {
            return;
        };

        unsafe { dealloc(ptr as *mut u8, layout) };
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn guest_realloc(
        ptr: u32,
        old_size: u32,
        align: u32,
        new_size: u32,
    ) -> u32 {
        let ptr = ptr as *mut u8;
        let old_size = old_size as usize;
        let align = align as usize;
        let new_size = new_size as usize;

        let Ok(layout) = Layout::from_size_align(old_size, align) else {
            return 0;
        };

        let new_ptr = unsafe { realloc(ptr as *mut u8, layout, new_size) };

        new_ptr.addr() as u32
    }
}
