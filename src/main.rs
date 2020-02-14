use deno_core::*;
use futures::executor::block_on;
use std::option::Option;
use futures::future::FutureExt;
use futures::future;

fn main() {
    let future = async_main();
    block_on(future);
}

async fn async_main() {
    let js_source = include_str!("cozal.js");

    let startup_data = StartupData::Script(Script {
        source: js_source,
        filename: "cozal.js",
    });

    let isolate = deno_core::Isolate::new(startup_data, false);
    let _op_id = isolate.register_op("printSync", print_from_rust_sync_op);
    let _op_id = isolate.register_op("printAsync", print_from_rust_async_op);
  
    let result = isolate.await;
    js_check(result);
}

fn js_check(r: Result<(), ErrBox>) {
    if let Err(e) = r {
        panic!(e.to_string());
    }
}

fn print_from_rust_sync_op(_control: &[u8], _zero_copy: Option<ZeroCopyBuf>) -> CoreOp {
    let vec = vec![42u8, 0, 0, 0];
    let buf = vec.into_boxed_slice();
    Op::Sync(buf)
}

fn print_from_rust_async_op(control: &[u8], _zero_copy: Option<ZeroCopyBuf>) -> CoreOp {
    // control should contain param small enough for cloning, so just do it.
    // control buf is temporary borrowed from Deno here, so to keep a reference
    // of it async, we have copy it.
    // async fn requires ownership or static lifetime of reference anyways. 
    let future = print_from_rust(control.to_owned(), _zero_copy);
    let future = future.then(|x| future::ready(x.map(|v| v.into_boxed_slice())));
    Op::Async(future.boxed())
}

async fn print_from_rust(_control: Vec<u8>, _zero_copy: Option<ZeroCopyBuf>) -> Result<Vec<u8>, CoreError> {
    Ok(vec![43u8, 0, 0, 0])
}