use std::alloc::Layout;

use crate::{center::CrimeCenter, config::create_config, context::CrimeCtx};

mod center;
mod config;
mod context;
mod host_calls;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();

    let crime_center = CrimeCenter::new(&args[0], create_config()).expect("crime center");
    let mut crime_instance = crime_center.instance().await.expect("instance");

    let layout = Layout::from_size_align(16, 8).unwrap();
    let new_layout = Layout::from_size_align(32, 8).unwrap();

    // Allocate a guest buffer, and fill it with 'A'.
    let mut buf = crime_instance.call_alloc(layout).await.unwrap().unwrap();
    let buf_slice = unsafe { buf.as_mut().await };
    buf_slice.fill(b'A');

    // Reallocate the guest buffer, and fill the new space with 'B'.
    let mut buf = crime_instance
        .call_realloc(buf, new_layout)
        .await
        .unwrap()
        .unwrap();
    let buf_slice = unsafe { buf.as_mut().await };
    buf_slice[16..].fill(b'B');

    // Pass the first 32 bytes to the guest program.
    let _l = crime_instance.call_enter(&buf, 32).await.unwrap();

    // Free the guest buffer.
    crime_instance.call_dealloc(buf).await.unwrap();

    // Try allocating some larger chunks.
    let big_layout = Layout::from_size_align(1024 * 1024 * 1024, 8).unwrap();
    let big_buf_1 = crime_instance
        .call_alloc(big_layout)
        .await
        .unwrap()
        .unwrap();
    dbg!(big_buf_1);
    let big_buf_2 = crime_instance
        .call_alloc(big_layout)
        .await
        .unwrap()
        .unwrap();
    dbg!(big_buf_2);
    let big_buf_3 = crime_instance
        .call_alloc(big_layout)
        .await
        .unwrap()
        .unwrap();
    dbg!(big_buf_3);

    // This will overflow because we'll have exhausted all possible memory in
    // the guest.
    let big_buf_4 = crime_instance.call_alloc(big_layout).await.unwrap();
    dbg!(big_buf_4);
}
