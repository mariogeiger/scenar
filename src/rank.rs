//! Ranks: keys that order siblings and that always leave room between two
//! of them, so that a block moves or arrives by rewriting its own key alone.
//!
//! A rank is the fraction 0.d₁d₂…dₙ in base 62, written with the digits
//! `0-9A-Za-z`, whose ASCII order is their value's order: comparing ranks as
//! strings compares the fractions. No rank ends in `0`, so there is always a
//! rank below any other, and between any two.

const DIGITS: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const BASE: u8 = 62;

fn value(digit: u8) -> u8 {
    DIGITS.iter().position(|&d| d == digit).expect("a rank holds only base-62 digits") as u8
}

/// A rank strictly between `low` and `high`, either missing meaning beyond
/// the ends; `low` must be below `high`.
pub fn between(low: Option<&str>, high: Option<&str>) -> String {
    let low: Vec<u8> = low.unwrap_or("").bytes().map(value).collect();
    let high: Option<Vec<u8>> = high.map(|h| h.bytes().map(value).collect());
    midpoint(&low, high.as_deref()).into_iter().map(|v| DIGITS[v as usize] as char).collect()
}

/// The digits of a fraction strictly between `low` and `high` (one when
/// missing), both without trailing zeros, `low` below `high`.
fn midpoint(low: &[u8], high: Option<&[u8]>) -> Vec<u8> {
    let at = |d: &[u8], i: usize| d.get(i).copied().unwrap_or(0);
    if let Some(high) = high {
        // Digits they share, low padded with zeros, belong to the answer.
        let shared = (0..high.len()).take_while(|&i| at(low, i) == high[i]).count();
        if shared > 0 {
            let mut out = high[..shared].to_vec();
            out.extend(midpoint(low.get(shared..).unwrap_or(&[]), Some(&high[shared..])));
            return out;
        }
    }
    let (l, h) = (at(low, 0), high.map_or(BASE, |h| h[0]));
    if h - l > 1 {
        // At an open end, one step, so that keys added there grow slowly.
        let v = match (low.is_empty(), high) {
            (true, None) => (l + h) / 2,
            (_, None) => l + 1,
            (true, Some(_)) => h - 1,
            _ => (l + h) / 2,
        };
        return vec![v];
    }
    // Adjacent first digits: high's first digit alone is below high when
    // high goes on, else the answer extends low.
    match high {
        Some(high) if high.len() > 1 => vec![high[0]],
        _ => {
            let mut out = vec![l];
            out.extend(midpoint(low.get(1..).unwrap_or(&[]), None));
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(low: Option<&str>, high: Option<&str>) -> String {
        let m = between(low, high);
        assert!(low.is_none_or(|l| l < m.as_str()), "{low:?} < {m}");
        assert!(high.is_none_or(|h| m.as_str() < h), "{m} < {high:?}");
        assert!(!m.ends_with('0') && !m.is_empty());
        m
    }

    #[test]
    fn between_the_ends_and_neighbours() {
        assert_eq!(check(None, None), "V");
        check(Some("V"), None);
        check(None, Some("V"));
        check(Some("1"), Some("2"));
        check(Some("1"), Some("11"));
        check(Some("09"), Some("1"));
        check(Some("z"), None);
        check(None, Some("01"));
        check(Some("zz"), None);
        check(Some("V"), Some("V1"));
    }

    #[test]
    fn repeated_insertions_stay_ordered_and_short() {
        // Always at the front, always at the back, and always in one gap,
        // narrowing it from either side in turn.
        let mut front = vec![check(None, None)];
        let mut back = front.clone();
        let (mut low, mut high) = (check(None, None), String::new());
        high.push_str(&check(Some(&low), None));
        for i in 0..200 {
            front.insert(0, check(None, Some(&front[0])));
            back.push(check(Some(back.last().unwrap()), None));
            let m = check(Some(&low), Some(&high));
            if i % 2 == 0 { low = m } else { high = m }
        }
        assert!(front.windows(2).all(|w| w[0] < w[1]) && back.windows(2).all(|w| w[0] < w[1]));
        assert!(back.last().unwrap().len() <= 8 && front[0].len() <= 8);
    }
}
