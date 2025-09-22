use wasmtime::{Caller, Linker};

use crate::CrimeCtx;

pub fn add_to_linker(linker: &mut Linker<CrimeCtx>) -> wasmtime::Result<()> {
    linker.func_wrap("wasm_crimes", "get", get)?;
    linker.func_wrap("wasm_crimes", "put", put)?;
    Ok(())
}

fn get(caller: Caller<'_, CrimeCtx>) -> u32 {
    caller.data().get()
}

fn put(mut caller: Caller<'_, CrimeCtx>, v: u32) -> u32 {
    caller.data_mut().put(v)
}
