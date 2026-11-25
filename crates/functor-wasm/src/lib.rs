use std::cell::RefCell;

thread_local! {
    static RESULT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

#[no_mangle]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::<u8>::with_capacity(len.max(1));
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

#[no_mangle]
pub unsafe extern "C" fn dealloc(ptr: *mut u8, len: usize) {
    drop(Vec::from_raw_parts(ptr, 0, len.max(1)));
}

unsafe fn input(ptr: *const u8, len: usize) -> String {
    let bytes = std::slice::from_raw_parts(ptr, len);
    String::from_utf8_lossy(bytes).into_owned()
}

fn set_result(s: String) -> usize {
    RESULT.with(|r| {
        let mut r = r.borrow_mut();
        *r = s.into_bytes();
        r.len()
    })
}

#[no_mangle]
pub extern "C" fn result_ptr() -> *const u8 {
    RESULT.with(|r| r.borrow().as_ptr())
}

#[no_mangle]
pub extern "C" fn result_len() -> usize {
    RESULT.with(|r| r.borrow().len())
}

#[no_mangle]
pub unsafe extern "C" fn compile(ptr: *const u8, len: usize) -> usize {
    let src = input(ptr, len);
    set_result(functor_paper::compile(&src).to_json())
}

#[no_mangle]
pub unsafe extern "C" fn compile_width(ptr: *const u8, len: usize, width_pt: f64) -> usize {
    let src = input(ptr, len);
    let w = if width_pt > 0.0 { Some(width_pt) } else { None };
    set_result(functor_paper::compile_width(&src, w).to_json())
}

#[no_mangle]
pub unsafe extern "C" fn compile_pdf(ptr: *const u8, len: usize) -> usize {
    let src = input(ptr, len);
    let out = functor_paper::compile_pdf(&src);
    RESULT.with(|r| {
        let mut r = r.borrow_mut();
        *r = out.pdf;
        r.len()
    })
}

#[no_mangle]
pub unsafe extern "C" fn glyph(ptr: *const u8, len: usize) -> usize {
    let recipe = input(ptr, len);
    set_result(functor_paper::glyph_svg(&recipe).unwrap_or_default())
}

#[no_mangle]
pub extern "C" fn symbols() -> usize {
    set_result(functor_paper::symbols_json())
}

#[no_mangle]
pub extern "C" fn cache_export() -> usize {
    let dirty = functor_paper::glyph::take_dirty();
    set_result(if dirty { functor_paper::glyph::export_cache() } else { String::new() })
}

#[no_mangle]
pub unsafe extern "C" fn cache_import(ptr: *const u8, len: usize) -> usize {
    functor_paper::glyph::import_cache(&input(ptr, len))
}
