use swift_check::{any, arch::load, eq, ensure, range, find, for_all_ensure, search, for_all_ensure_ct, one_of};
use criterion::{Criterion, Throughput, criterion_group, criterion_main, black_box};
use swift_check::not;
use swift_check::require::check;
use swift_check::{requirement, requirements};

fn bench_aligned(c: &mut Criterion) {
    let inp = b"hello world 1234";
    let mut g = c.benchmark_group("simd-aligned");
    let data = load(inp);

    g.throughput(Throughput::Bytes(inp.len() as u64));

    g.bench_function("simd/ensure-for-1", |b| {
        b.iter(|| {
            let legal = ensure!(black_box(data), eq(b'4'));
            black_box(legal)
        })
    });
    g.bench_function("simd/find_at_end", |b| {
        b.iter(|| {
            let legal = find!(black_box(data), eq(b'4'));
            assert_eq!(legal, Some(15));
        })
    });
    g.bench_function("simd/ensure-for-2", |b| {
        b.iter(|| {
            let legal = ensure!(black_box(data), any!(eq(b'h'), eq(b'1')));
            black_box(legal)
        })
    });
    g.bench_function("simd/ensure-for-2-any!", |b| {
        b.iter(|| {
            let legal = ensure!(black_box(data), any!(eq(b'h'), eq(b'1')));
            black_box(legal)
        })
    });
    g.bench_function("std/ensure-for-2", |b| {
        b.iter(|| {
            let mut legal = true;
            for byte in black_box(inp) {
                legal &= matches!(black_box(byte), b'h' | b'1');
            }
            black_box(legal)
        })
    });
    g.bench_function("simd/is-lowercase-alphabet", |b| {
        b.iter(|| {
            let legal = ensure!(black_box(data), range!(b'a'..b'z'));
            black_box(legal)
        })
    });
    g.bench_function("std/is-lowercase-alphabet", |b| {
        b.iter(|| {
            let legal = black_box(inp).iter().all(|&c| matches!(c, b'a'..=b'z'));
            black_box(legal)
        })
    });
    g.bench_function("simd/is-alphabet", |b| {
        b.iter(|| {
            let legal = ensure!(black_box(data), any!(range!(b'a'..=b'z'), range!(b'A'..=b'Z')));
            black_box(legal)
        })
    });
    g.bench_function("std/is-alphabet", |b| {
        b.iter(|| {
            let legal = black_box(inp).iter().all(|&c| matches!(c, b'a'..=b'z' | b'A'..=b'Z'));
            black_box(legal)
        })
    });
}

fn bench_multi(c: &mut Criterion) {
    let input = b"Hello world I am an input with the numbers at the end, for this bench my \
    length is divisible by                              016";

    let mut g = c.benchmark_group("simd-multi");
    g.throughput(Throughput::Bytes(input.len() as u64));

    g.bench_function("simd/ensure-range-ct", |b| {
        b.iter(|| {
            let res = for_all_ensure_ct(black_box(input), range!(b'0'..=b'z'));
            black_box(res)
        })
    });

    g.bench_function("simd/find_any_3", |b| {
        b.iter(|| {
            let res = search(black_box(input), any!(eq(b'0'), eq(b'1'), eq(b'6')));
            black_box(res);
        })
    });

    g.bench_function("memchr/memchr3", |b| {
        b.iter(|| {
            let res = memchr::memchr3(b'0', b'1', b'6', black_box(input));
            black_box(res)
        })
    });

    g.bench_function("simd/find-at-end", |b| {
        b.iter(|| {
            assert_eq!(search(black_box(input), eq(b'6')), Some(127));
        })
    });

    g.bench_function("memchr/find-at-end", |b| {
        b.iter(|| {
            assert_eq!(memchr::memchr(b'6', black_box(input)), Some(127));
        })
    });
}

fn bench_remainder(c: &mut Criterion) {
    let input = b"                         Hello world";

    let mut g = c.benchmark_group("simd-remainder");
    g.throughput(Throughput::Bytes(input.len() as u64));

    g.bench_function("simd/ensure-all", |b| {
        b.iter(|| {
            let res = for_all_ensure(black_box(input), not(eq(b'z')));
            black_box(res);
        })
    });

    g.bench_function("simd/find-at-end", |b| {
        b.iter(|| {
            assert_eq!(search(black_box(input), eq(b'd')), Some(35));
        })
    });

    g.bench_function("memchr/find-at-end", |b| {
        b.iter(|| {
            assert_eq!(memchr::memchr(b'd', black_box(input)), Some(35));
        })
    });
}

fn bench_partial(c: &mut Criterion) {
    let input = b"hello";
    let mut g = c.benchmark_group("partial-loads");
    g.throughput(Throughput::Bytes(input.len() as u64));

    g.bench_function("partial-ensure", |b| {
        b.iter(|| {
            let res = for_all_ensure(black_box(input), range!(b'a'..=b'z'));
            black_box(res)
        })
    });

    g.bench_function("partial-search", |b| {
        b.iter(|| {
            let res = search(black_box(input), eq(b'o'));
            black_box(res);
        })
    });
}

requirement!(uppercase => range!(b'A'..=b'Z') =>! "needs uppercase!");
requirement!(lowercase => range!(b'a'..=b'z') =>! "needs lowercase!");
requirement!(numeric => range!(b'0'..=b'9') =>! "needs number!");
requirement!(question_mark => eq(b'?') =>! "needs a question mark!");

fn bench_massive(c: &mut Criterion) {
    let mut input = [0u8; 524288];

    let mut g = c.benchmark_group("massive");
    g.throughput(Throughput::Bytes(input.len() as u64));

    input[input.len() - 1] = b'5';



    g.bench_function("find-num-at-end", |b| {
        b.iter(|| {
            let Some(pos) = search(black_box(&input), range!(b'0'..=b'9')) else {
                panic!("input ended with 5")
            };
            assert_eq!(input[pos], b'5');
        })
    });

    g.bench_function("nom/find-num-at-end", |b| {
        b.iter(|| {
            let Ok((rem, _res)) = nom::bytes::complete::take_till::<_, &[u8], nom::error::Error<&[u8]>>(
                nom::AsChar::is_dec_digit
            )(black_box(input.as_slice())) else {
                panic!("input ended with 5")
            };
            assert_eq!(rem[0], b'5');
        })
    });

    g.bench_function("no-find", |b| {
        b.iter(|| {
            let res = search(black_box(&input), eq(1));
            black_box(res);
        })
    });

    g.bench_function("memchr/no-find", |b| {
        b.iter(|| {
            let res = memchr::memchr(1, black_box(&input));
            black_box(res);
        })
    });

    input[input.len() - 1] = 1;

    g.bench_function("find-at-end", |b| {
        b.iter(|| {
            let res = search(black_box(&input), eq(1));
            black_box(res);
        })
    });

    g.bench_function("one-of-4-ensure-ct", |b| {
        b.iter(|| {
            let res = for_all_ensure_ct(black_box(&input), one_of!(
                eq(0), eq(1), eq(2), eq(3)
            ));
            assert!(res);
        })
    });

    g.bench_function("complex-requirements-experimental", |b| {
        b.iter(|| {
            let res = check(
                black_box(&input),
                requirements!([uppercase, lowercase, numeric, question_mark])
            );
            black_box(res);
        })
    });
}

criterion_group!(benches, bench_massive, bench_multi, bench_remainder, bench_partial, bench_aligned);
criterion_main!(benches);