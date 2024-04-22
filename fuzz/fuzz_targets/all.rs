#![no_main]

use libfuzzer_sys::fuzz_target;
use swift_check::*;

fuzz_target!(|data: &[u8]| {

    let res = for_all_ensure(data, all!(
        eq(b'1'), not(eq(b'l')),
        xor(range!(b'0'..b'9'), range!(b'0'..=b'4')),
        any!(eq(b'1'), eq(b'2'), eq(b'2')),
        and(eq(b'1'), eq(b'1')),
        range!(> b'9'), range!(< b'9'),
        range!(>= b'9'), range!(<= b'9')
    ));

    core::hint::black_box(res);
});
