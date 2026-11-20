//! math.rs — `Math.exp` and `Math.log2` as node's V8 computes them: fdlibm's `exp` and FreeBSD's `log2`,
//! from V8 13.6's `src/base/ieee754.cc`, operation for operation. The waveform sidecar's band coefficients and
//! tempo prior go through them. What a reader gets wrong: Rust's `f64::exp` is the platform's libm, which may
//! differ by an ulp — and a later V8 (LLVM's libc) may too; `analysis-vectors.json` says which (`docs/mediatek.md`).

fn high(x: f64) -> i32 {
    (x.to_bits() >> 32) as i32
}

fn low(x: f64) -> u32 {
    x.to_bits() as u32
}

fn words(hi: u32, lo: u32) -> f64 {
    f64::from_bits((u64::from(hi) << 32) | u64::from(lo))
}

fn with_high(x: f64, hi: u32) -> f64 {
    words(hi, low(x))
}

fn with_low(x: f64, lo: u32) -> f64 {
    words(high(x) as u32, lo)
}

/// `Math.exp(x)`, V8 13.6's `ieee754::exp`.
pub fn exp(x: f64) -> f64 {
    const HALF: [f64; 2] = [0.5, -0.5];
    const O_THRESHOLD: f64 = f64::from_bits(0x4086_2E42_FEFA_39EF);
    const U_THRESHOLD: f64 = f64::from_bits(0xC087_4910_D52D_3051);
    const LN2_HI: [f64; 2] = [
        f64::from_bits(0x3FE6_2E42_FEE0_0000),
        f64::from_bits(0xBFE6_2E42_FEE0_0000),
    ];
    const LN2_LO: [f64; 2] = [
        f64::from_bits(0x3DEA_39EF_3579_3C76),
        f64::from_bits(0xBDEA_39EF_3579_3C76),
    ];
    const INVLN2: f64 = f64::from_bits(0x3FF7_1547_652B_82FE);
    const P1: f64 = f64::from_bits(0x3FC5_5555_5555_553E);
    const P2: f64 = f64::from_bits(0xBF66_C16C_16BE_BD93);
    const P3: f64 = f64::from_bits(0x3F11_566A_AF25_DE2C);
    const P4: f64 = f64::from_bits(0xBEBB_BD41_C5D2_6BF1);
    const P5: f64 = f64::from_bits(0x3E66_3769_72BE_A4D0);
    const E: f64 = f64::from_bits(0x4005_BF0A_8B14_5769);
    const HUGE: f64 = 1.0e300;
    const TWOM1000: f64 = f64::from_bits(0x0170_0000_0000_0000);
    const TWO1023: f64 = f64::from_bits(0x7FE0_0000_0000_0000);

    let mut x = x;
    let mut hx = high(x) as u32;
    let xsb = ((hx >> 31) & 1) as usize;
    hx &= 0x7FFF_FFFF;
    let (mut hi, mut lo, mut k) = (0.0f64, 0.0f64, 0i32);

    if hx >= 0x4086_2E42 {
        if hx >= 0x7FF0_0000 {
            if ((hx & 0xF_FFFF) | low(x)) != 0 {
                return x + x;
            }
            return if xsb == 0 { x } else { 0.0 };
        }
        if x > O_THRESHOLD {
            return HUGE * HUGE;
        }
        if x < U_THRESHOLD {
            return TWOM1000 * TWOM1000;
        }
    }

    if hx > 0x3FD6_2E42 {
        if hx < 0x3FF0_A2B2 {
            // V8's own special case: the reduction below gets exp(1)'s last bit wrong.
            if x == 1.0 {
                return E;
            }
            hi = x - LN2_HI[xsb];
            lo = LN2_LO[xsb];
            k = 1 - xsb as i32 - xsb as i32;
        } else {
            k = (INVLN2 * x + HALF[xsb]) as i32;
            let t = f64::from(k);
            hi = x - t * LN2_HI[0];
            lo = t * LN2_LO[0];
        }
        x = hi - lo;
    } else if hx < 0x3E30_0000 {
        if HUGE + x > 1.0 {
            return 1.0 + x;
        }
    } else {
        k = 0;
    }

    let t = x * x;
    let twopk = if k >= -1021 {
        words(0x3FF0_0000u32.wrapping_add((k as u32) << 20), 0)
    } else {
        words(0x3FF0_0000u32.wrapping_add(((k + 1000) as u32) << 20), 0)
    };
    let c = x - t * (P1 + t * (P2 + t * (P3 + t * (P4 + t * P5))));
    if k == 0 {
        return 1.0 - ((x * c) / (c - 2.0) - x);
    }
    let y = 1.0 - ((lo - (x * c) / (2.0 - c)) - hi);
    if k >= -1021 {
        if k == 1024 {
            return y * 2.0 * TWO1023;
        }
        y * twopk
    } else {
        y * twopk * TWOM1000
    }
}

/// `log(1+f) - f` for 1+f in ~[sqrt(2)/2, sqrt(2)], fdlibm's `k_log1p`.
fn k_log1p(f: f64) -> f64 {
    const LG1: f64 = f64::from_bits(0x3FE5_5555_5555_5593);
    const LG2: f64 = f64::from_bits(0x3FD9_9999_9997_FA04);
    const LG3: f64 = f64::from_bits(0x3FD2_4924_9422_9359);
    const LG4: f64 = f64::from_bits(0x3FCC_71C5_1D8E_78AF);
    const LG5: f64 = f64::from_bits(0x3FC7_4664_96CB_03DE);
    const LG6: f64 = f64::from_bits(0x3FC3_9A09_D078_C69F);
    const LG7: f64 = f64::from_bits(0x3FC2_F112_DF3E_5244);
    let s = f / (2.0 + f);
    let z = s * s;
    let w = z * z;
    let t1 = w * (LG2 + w * (LG4 + w * LG6));
    let t2 = z * (LG1 + w * (LG3 + w * (LG5 + w * LG7)));
    let r = t2 + t1;
    let hfsq = 0.5 * f * f;
    s * (hfsq + r)
}

/// `Math.log2(x)`, V8 13.6's `ieee754::log2`.
pub fn log2(x: f64) -> f64 {
    const TWO54: f64 = f64::from_bits(0x4350_0000_0000_0000);
    const IVLN2HI: f64 = f64::from_bits(0x3FF7_1547_6520_0000);
    const IVLN2LO: f64 = f64::from_bits(0x3DE7_05FC_2EEF_A200);

    let mut x = x;
    let mut hx = high(x);
    let lx = low(x);
    let mut k: i32 = 0;
    if hx < 0x0010_0000 {
        if ((hx & 0x7FFF_FFFF) as u32 | lx) == 0 {
            return f64::NEG_INFINITY;
        }
        if hx < 0 {
            return f64::NAN;
        }
        k -= 54;
        x *= TWO54;
        hx = high(x);
    }
    if hx >= 0x7FF0_0000 {
        return x + x;
    }
    if hx == 0x3FF0_0000 && lx == 0 {
        return 0.0;
    }
    k += (hx >> 20) - 1023;
    hx &= 0x000F_FFFF;
    let i = (hx + 0x95F64) & 0x10_0000;
    x = with_high(x, (hx | (i ^ 0x3FF0_0000)) as u32);
    k += i >> 20;
    let y = f64::from(k);
    let f = x - 1.0;
    let hfsq = 0.5 * f * f;
    let r = k_log1p(f);
    let hi = with_low(f - hfsq, 0);
    let lo = (f - hi) - hfsq + r;
    let mut val_hi = hi * IVLN2HI;
    let mut val_lo = (lo + hi) * IVLN2LO + lo * IVLN2HI;
    let w = y + val_hi;
    val_lo += (y - w) + val_hi;
    val_hi = w;
    val_lo + val_hi
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_the_easy_values() {
        assert_eq!(exp(0.0), 1.0);
        assert_eq!(exp(1.0), std::f64::consts::E);
        assert_eq!(exp(f64::NEG_INFINITY), 0.0);
        assert!(exp(f64::NAN).is_nan());
        assert_eq!(exp(1000.0), f64::INFINITY);
        assert_eq!(log2(8.0), 3.0);
        assert_eq!(log2(1.0), 0.0);
        assert_eq!(log2(0.0), f64::NEG_INFINITY);
        assert!(log2(-1.0).is_nan());
        assert_eq!(log2(0.5), -1.0);
    }
}
