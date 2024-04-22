
macro_rules! cfg_neon {
    ($($item:item)*) => {
        $(
            #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))]
            $item
        )*
    };
}

macro_rules! cfg_sse {
    ($($item:item)*) => {
        $(
            #[cfg(all(feature = "simd", target_arch = "x86_64", target_feature = "sse2"))]
            $item
        )*
    };
}

macro_rules! cfg_simd128 {
    ($($item:item)*) => {
        $(
            #[cfg(all(feature = "simd", target_family = "wasm", target_feature = "simd128"))]
            $item
        )*
    };
}

macro_rules! cfg_fallback {
    ($($item:item)*) => {
        $(
            #[cfg(any(not(feature = "simd"), not(any(
                all(target_arch = "x86_64", target_feature = "sse2"),
                all(target_arch = "aarch64", target_feature = "neon"),
                all(target_family = "wasm", target_feature = "simd128")
            ))))]
            $item
        )*
    };
}

macro_rules! cfg_i8 {
    ($($item:item)*) => {
        $(
            #[cfg(all(feature = "simd", any(
                all(target_arch = "x86_64", target_feature = "sse2"),
                all(target_family = "wasm", target_feature = "simd128")
            )))]
            $item
        )*
    };
}

macro_rules! cfg_u8 {
    ($($item:item)*) => {
        $(
            #[cfg(not(all(feature = "simd", any(
                all(target_arch = "x86_64", target_feature = "sse2"),
                all(target_family = "wasm", target_feature = "simd128")
            ))))]
            $item
        )*
    };
}

macro_rules! cfg_simd {
    ($($item:item)*) => {
        $(
            #[cfg(not(any(not(feature = "simd"), not(any(
                all(target_arch = "x86_64", target_feature = "sse2"),
                all(target_arch = "aarch64", target_feature = "neon"),
                all(target_family = "wasm", target_feature = "simd128")
            )))))]
            $item
        )*
    };
}