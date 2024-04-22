use super::Vector;

macro_rules! scan_all {
    (
        $data:ident, $idx:ident,
        |$chunk:ident| => $do:expr,
        |$partial:ident| => $handle_partial:expr; or $or:expr
    ) => {{
        while $idx <= $data.len() - super::WIDTH {
            let $chunk = &$data[$idx..$idx + super::WIDTH];
            $do;
            $idx += super::WIDTH;
        }
        match $data.len() - $idx {
            0 => $or,
            offset => {
                $idx -= super::WIDTH - offset;
                let $partial = &$data[$idx..$idx + super::WIDTH];
                $handle_partial
            }
        }
    }};
}

#[inline(always)]
pub unsafe fn for_all_ensure_ct(data: &[u8], cond: impl Fn(Vector) -> Vector, res: &mut bool) {
    let mut idx = 0;
    scan_all!(
        data, idx,
        |chunk| => *res &= crate::ensure!(super::load_unchecked(chunk), cond),
        |partial| => *res &= crate::ensure!(super::load_unchecked(partial), cond); or {}
    );
}

#[inline(always)]
pub unsafe fn for_all_ensure(data: &[u8], cond: impl Fn(Vector) -> Vector) -> bool {
    let mut idx = 0;
    scan_all!(
        data, idx,
        |chunk| => if !crate::ensure!(super::load_unchecked(chunk), cond) { return false },
        |partial| => crate::ensure!(super::load_unchecked(partial), cond); or true
    )
}

#[inline(always)]
pub unsafe fn search(data: &[u8], cond: impl Fn(Vector) -> Vector) -> Option<usize> {
    let mut idx = 0;
    scan_all!(
        data, idx,
        |chunk| => if let Some(position) = crate::find!(super::load_unchecked(chunk), cond) {
            return Some(position as usize + idx)
        },
        |partial| => crate::find!(super::load_unchecked(partial), cond)
            .map(|pos| pos as usize + idx); or None
    )
}