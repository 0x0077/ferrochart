//! The synthetic instruments. Names are generic (asset class, not a listed
//! company): every price on the site is synthetic.

use serde::Serialize;

use super::noise::{Path, seed_of};

pub const DAY_MS: i64 = 86_400_000;
pub const MINUTE_MS: i64 = 60_000;

/// A trading window: start (minutes after UTC midnight) and length.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Window {
    pub start_min: i64,
    pub len_min: i64,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Market {
    Crypto,
    Future,
    Stock,
    Fx,
    Metal,
    AShare,
    Rate,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Symbol {
    pub id: &'static str,
    pub name: &'static str,
    pub market: Market,
    /// Price level the path moves around.
    pub base: f64,
    /// Std of one-day log returns.
    pub daily_vol: f64,
    pub tick: f64,
    pub price_decimals: u32,
    pub size_decimals: u32,
    /// Average trades per second at normal activity.
    pub trades_per_sec: f64,
    /// Average trade size.
    pub avg_size: f64,
    /// Trading windows (UTC); empty: around the clock.
    pub sessions: &'static [Window],
    /// No trading on Saturday and Sunday (UTC).
    pub weekdays_only: bool,
    /// Rising candles are red (Chinese convention), for the examples.
    pub red_up: bool,
    /// Moves with another symbol: its id and the weight of its moves.
    #[serde(skip)]
    pub link: Option<(&'static str, f64)>,
}

const US_CASH: &[Window] = &[Window {
    start_min: 13 * 60 + 30,
    len_min: 390,
}];
const HK: &[Window] = &[
    Window {
        start_min: 60 + 30,
        len_min: 150,
    },
    Window {
        start_min: 5 * 60,
        len_min: 180,
    },
];
const CN_A: &[Window] = &[
    Window {
        start_min: 60 + 30,
        len_min: 120,
    },
    Window {
        start_min: 5 * 60,
        len_min: 120,
    },
];

macro_rules! sym {
    ($id:literal, $name:literal, $market:ident, $base:expr, $vol:expr, $tick:expr, $pd:expr, $sd:expr, $tps:expr, $size:expr, $sess:expr, $wd:expr, $red:expr) => {
        Symbol {
            id: $id,
            name: $name,
            market: Market::$market,
            base: $base,
            daily_vol: $vol,
            tick: $tick,
            price_decimals: $pd,
            size_decimals: $sd,
            trades_per_sec: $tps,
            avg_size: $size,
            sessions: $sess,
            weekdays_only: $wd,
            red_up: $red,
            link: None,
        }
    };
    ($id:literal, $name:literal, $market:ident, $base:expr, $vol:expr, $tick:expr, $pd:expr, $sd:expr, $tps:expr, $size:expr, $sess:expr, $wd:expr, $red:expr, link = ($parent:literal, $w:expr)) => {
        Symbol {
            link: Some(($parent, $w)),
            ..sym!(
                $id, $name, $market, $base, $vol, $tick, $pd, $sd, $tps, $size, $sess, $wd, $red
            )
        }
    };
}

pub static SYMBOLS: &[Symbol] = &[
    sym!(
        "BTCUSD",
        "Bitcoin / US dollar",
        Crypto,
        62_000.0,
        0.028,
        0.5,
        1,
        4,
        9.0,
        0.06,
        &[],
        false,
        false
    ),
    sym!(
        "ETHUSD",
        "Ether / US dollar",
        Crypto,
        3_100.0,
        0.034,
        0.05,
        2,
        3,
        7.0,
        0.9,
        &[],
        false,
        false
    ),
    sym!(
        "SOLUSD",
        "Solana / US dollar",
        Crypto,
        150.0,
        0.045,
        0.01,
        2,
        2,
        5.0,
        18.0,
        &[],
        false,
        false
    ),
    sym!(
        "MEME",
        "Memecoin (illustrative)",
        Crypto,
        0.0042,
        0.14,
        0.000_001,
        6,
        0,
        3.0,
        250_000.0,
        &[],
        false,
        false
    ),
    sym!(
        "ES",
        "Equity index future",
        Future,
        5_200.0,
        0.011,
        0.25,
        2,
        0,
        6.0,
        2.0,
        US_CASH,
        true,
        false
    ),
    sym!(
        "NQ",
        "Tech index future",
        Future,
        18_300.0,
        0.014,
        0.25,
        2,
        0,
        4.0,
        1.5,
        US_CASH,
        true,
        false
    ),
    sym!(
        "ACME",
        "Large-cap stock (illustrative)",
        Stock,
        210.0,
        0.017,
        0.01,
        2,
        0,
        5.0,
        120.0,
        US_CASH,
        true,
        false
    ),
    sym!(
        "NOVA",
        "Growth stock (illustrative)",
        Stock,
        120.0,
        0.03,
        0.01,
        2,
        0,
        6.0,
        150.0,
        US_CASH,
        true,
        false
    ),
    sym!(
        "EURUSD",
        "Euro / US dollar",
        Fx,
        1.08,
        0.005,
        0.000_01,
        5,
        0,
        4.0,
        100_000.0,
        &[],
        true,
        false
    ),
    sym!(
        "XAUUSD",
        "Gold / US dollar",
        Metal,
        2_400.0,
        0.01,
        0.01,
        2,
        0,
        3.0,
        5.0,
        &[],
        true,
        false
    ),
    sym!(
        "CNA",
        "A-share stock (illustrative)",
        AShare,
        18.5,
        0.022,
        0.01,
        2,
        0,
        3.0,
        1_200.0,
        CN_A,
        true,
        true
    ),
    sym!(
        "CNAH",
        "H-share of the A-share stock (illustrative)",
        Stock,
        16.2,
        0.024,
        0.01,
        2,
        0,
        2.0,
        2_000.0,
        HK,
        true,
        false,
        link = ("CNA", 0.8)
    ),
    sym!(
        "CN300",
        "A-share index (illustrative)",
        AShare,
        3_850.0,
        0.012,
        0.01,
        2,
        0,
        4.0,
        500.0,
        CN_A,
        true,
        true,
        link = ("CNA", 0.5)
    ),
    sym!(
        "CL",
        "Crude oil future",
        Future,
        78.0,
        0.02,
        0.01,
        2,
        0,
        5.0,
        3.0,
        &[],
        true,
        false
    ),
    sym!(
        "RB",
        "Gasoline future",
        Future,
        2.45,
        0.021,
        0.0001,
        4,
        0,
        2.0,
        2.0,
        &[],
        true,
        false,
        link = ("CL", 0.9)
    ),
    sym!(
        "HO",
        "Heating oil future",
        Future,
        2.6,
        0.02,
        0.0001,
        4,
        0,
        2.0,
        2.0,
        &[],
        true,
        false,
        link = ("CL", 0.9)
    ),
    sym!(
        "NG",
        "Natural gas future",
        Future,
        2.9,
        0.035,
        0.001,
        3,
        0,
        3.0,
        2.0,
        &[],
        true,
        false
    ),
    sym!(
        "ZS",
        "Soybean future",
        Future,
        1_150.0,
        0.013,
        0.25,
        2,
        0,
        3.0,
        2.0,
        &[],
        true,
        false
    ),
    sym!(
        "ZM",
        "Soybean meal future",
        Future,
        340.0,
        0.015,
        0.1,
        1,
        0,
        2.0,
        2.0,
        &[],
        true,
        false,
        link = ("ZS", 0.85)
    ),
    sym!(
        "ZL",
        "Soybean oil future",
        Future,
        44.0,
        0.017,
        0.01,
        2,
        0,
        2.0,
        2.0,
        &[],
        true,
        false,
        link = ("ZS", 0.75)
    ),
    sym!(
        "EUA",
        "Carbon allowance future",
        Future,
        68.0,
        0.025,
        0.01,
        2,
        0,
        2.0,
        5.0,
        &[],
        true,
        false
    ),
    sym!(
        "US10Y",
        "10-year yield, percent (illustrative)",
        Rate,
        4.2,
        0.018,
        0.001,
        3,
        0,
        2.0,
        1.0,
        &[],
        true,
        false
    ),
    sym!(
        "USDJPY",
        "US dollar / yen",
        Fx,
        152.0,
        0.006,
        0.001,
        3,
        0,
        4.0,
        100_000.0,
        &[],
        true,
        false
    ),
    sym!(
        "ETHBTC",
        "Ether / bitcoin",
        Crypto,
        0.05,
        0.025,
        0.000_01,
        5,
        2,
        3.0,
        2.0,
        &[],
        false,
        false,
        link = ("ETHUSD", 0.7)
    ),
];

pub fn find(id: &str) -> Option<&'static Symbol> {
    SYMBOLS.iter().find(|s| s.id.eq_ignore_ascii_case(id))
}

impl Symbol {
    pub fn path(&self) -> Path {
        let own = Path::new(seed_of(self.id), self.daily_vol);
        match self.link.and_then(|(id, w)| find(id).map(|p| (p, w))) {
            // One level only: a parent's own link is ignored.
            Some((p, w)) => own.linked(Path::new(seed_of(p.id), self.daily_vol), w),
            None => own,
        }
    }

    pub fn seed(&self) -> u64 {
        seed_of(self.id)
    }

    /// Whether `t_ms` is inside a trading window.
    pub fn in_session(&self, t_ms: i64) -> bool {
        if self.weekdays_only {
            // 1970-01-01 was a Thursday: day 0 → weekday 3 (Mon = 0).
            let weekday = (t_ms.div_euclid(DAY_MS) + 3).rem_euclid(7);
            if weekday >= 5 {
                return false;
            }
        }
        if self.sessions.is_empty() {
            return true;
        }
        let min = t_ms.rem_euclid(DAY_MS) / MINUTE_MS;
        self.sessions
            .iter()
            .any(|w| min >= w.start_min && min < w.start_min + w.len_min)
    }

    /// Share of the time the market is open.
    pub fn open_fraction(&self) -> f64 {
        let day = if self.sessions.is_empty() {
            1.0
        } else {
            self.sessions.iter().map(|w| w.len_min).sum::<i64>() as f64 / 1440.0
        };
        day * if self.weekdays_only { 5.0 / 7.0 } else { 1.0 }
    }

    /// Intraday activity (1 on average): U-shaped within a window, flat for
    /// markets open around the clock.
    pub fn activity(&self, t_ms: i64) -> f64 {
        let min = (t_ms.rem_euclid(DAY_MS) / 1000) as f64 / 60.0;
        let Some(w) = self
            .sessions
            .iter()
            .find(|w| min >= w.start_min as f64 && min < (w.start_min + w.len_min) as f64)
        else {
            return 1.0;
        };
        let x = (min - w.start_min as f64) / w.len_min as f64;
        (1.0 + 1.6 * ((-x / 0.07).exp() + (-(1.0 - x) / 0.09).exp())) / 1.25
    }

    pub fn price(&self, log: f64) -> f64 {
        self.base * log.exp()
    }

    pub fn round_price(&self, p: f64) -> f64 {
        round_to(p, self.price_decimals)
    }

    pub fn round_size(&self, s: f64) -> f64 {
        round_to(s, self.size_decimals)
    }
}

/// Rounds to `decimals` so that JSON prints the short decimal form.
pub fn round_to(x: f64, decimals: u32) -> f64 {
    let m = 10f64.powi(decimals as i32);
    (x * m).round() / m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sessions() {
        let es = find("es").map(|s| s.in_session(0));
        // 1970-01-01 00:00 UTC, Thursday, before the US open.
        assert_eq!(es, Some(false));
        let thu_15h = 15 * 3_600_000;
        assert_eq!(find("ES").map(|s| s.in_session(thu_15h)), Some(true));
        let sat_15h = 2 * DAY_MS + thu_15h;
        assert_eq!(find("ES").map(|s| s.in_session(sat_15h)), Some(false));
        assert_eq!(find("BTCUSD").map(|s| s.in_session(sat_15h)), Some(true));
        let lunch = 4 * 3_600_000;
        assert_eq!(find("CNA").map(|s| s.in_session(lunch)), Some(false));
    }

    #[test]
    fn ids_are_unique() {
        for (i, a) in SYMBOLS.iter().enumerate() {
            assert!(SYMBOLS[i + 1..].iter().all(|b| b.id != a.id));
        }
    }
}
