/**
 * @file
 * @author Edward A. Lee (eal@berkeley.edu)
 * @author Soroush Bateni (soroush@utdallas.edu)
 * @author Erling Jellum (erling.r.jellum@ntnu.no)
 * @author Chadlia Jerad (chadlia.jerad@ensi-uma.tn)
 * @author Chanhee Lee (chanheel@asu.edu)
 * @author Hokeun Kim (hokeun@asu.edu)
 * @copyright (c) 2020-2023, The University of California at Berkeley
 * License in [BSD 2-clause](..)
 * @brief Declarations for runtime infrastructure (RTI) for distributed Lingua Franca programs.
 * This file extends enclave.h with RTI features that are specific to federations and are not
 * used by scheduling enclaves.
 */

////////////////  Type definitions

/**
 * Time instant. Both physical and logical times are represented
 * using this typedef.
 */
pub type Instant = i64;

/**
 * Interval of time.
 */
pub type Interval = std::option::Option<i64>;

/**
 * Microstep instant.
 */
pub type Microstep = u32;

pub const NEVER: i64 = i64::MIN;
pub const FOREVER: i64 = i64::MAX;
pub const NEVER_MICROSTEP: u32 = 0;
pub const FOREVER_MICROSTEP: u32 = u32::MAX;

pub struct StartTime {
    start_time: Instant,
}

impl StartTime {
    pub fn new() -> StartTime {
        StartTime { start_time: NEVER }
    }

    pub fn start_time(&self) -> Instant {
        self.start_time
    }

    pub fn set_start_time(&mut self, start_time: Instant) {
        self.start_time = start_time;
    }
}

/**
 * A tag is a time, microstep pair.
 */
#[derive(Hash, Eq, PartialEq, Clone)]
pub struct Tag {
    time: Instant,
    microstep: Microstep,
}

////////////////  Functions

impl Tag {
    pub fn new(time: Instant, microstep: Microstep) -> Tag {
        Tag { time, microstep }
    }

    pub fn zero_tag() -> Tag {
        Tag {
            time: 0,
            microstep: 0,
        }
    }

    pub fn never_tag() -> Tag {
        Tag {
            time: NEVER,
            microstep: 0,
        }
    }

    pub fn forever_tag() -> Tag {
        Tag {
            time: FOREVER,
            microstep: FOREVER_MICROSTEP,
        }
    }

    pub fn time(&self) -> Instant {
        self.time.clone()
    }

    pub fn microstep(&self) -> Microstep {
        self.microstep.clone()
    }

    pub fn set_time(&mut self, time: i64) {
        self.time = time;
    }

    pub fn set_microstep(&mut self, microstep: u32) {
        self.microstep = microstep;
    }

    pub fn lf_tag_compare(tag1: &Tag, tag2: &Tag) -> i32 {
        let tag1_time = tag1.time();
        let tag2_time = tag2.time();
        let tag1_microstep = tag1.microstep();
        let tag2_microstep = tag2.microstep();
        if tag1_time < tag2_time {
            // println!("{} < {}", tag1_time, tag2_time);
            -1
        } else if tag1_time > tag2_time {
            // println!("{} > {}", tag1_time, tag2_time);
            1
        } else if tag1_microstep < tag2_microstep {
            // println!("{} and microstep < {}", tag1_microstep, tag2_microstep);
            -1
        } else if tag1_microstep > tag2_microstep {
            1
        } else {
            0
        }
    }

    pub fn lf_delay_tag(tag: &Tag, interval: Interval) -> Tag {
        if tag.time() == NEVER || interval < Some(0) {
            // println!(
            //     "tag.time() == i64::MIN || interval < Some(0),  (interval, time) = ({:?},{})",
            //     interval,
            //     tag.time()
            // );
            return tag.clone();
        }
        // Note that overflow in C is undefined for signed variables.
        if tag.time() >= FOREVER - interval.unwrap() {
            return Tag::forever_tag(); // Overflow.
        }
        let mut result = tag.clone();
        if interval == Some(0) {
            // Note that unsigned variables will wrap on overflow.
            // This is probably the only reasonable thing to do with overflowing
            // microsteps.
            result.set_microstep(result.microstep() + 1);
            // println!(
            //     "interval == 0,  (time, microstep) = ({},{})",
            //     result.time(),
            //     result.microstep()
            // );
        } else {
            // FIXME: Handle unwrap() properly.
            result.set_time(result.time() + interval.unwrap());
            result.set_microstep(0);
            println!(
                "result.set_time(result.time() + interval.unwrap()),  (time, microstep) = ({},{})",
                result.time(),
                result.microstep()
            );
        }
        result
    }

    pub fn lf_delay_strict(tag: &Tag, interval: Interval) -> Tag {
        let mut result = Self::lf_delay_tag(tag, interval);
        if interval != Some(0)
            && interval != Some(NEVER)
            && interval != Some(FOREVER)
            && result.time() != NEVER
            && result.time() != FOREVER
        {
            // println!("interval={:?}, result time={}", interval, result.time());
            result.set_time(result.time() - 1);
            result.set_microstep(u32::MAX);
        }
        // println!(
        //     "(time, microstep) = ({},{})",
        //     result.time(),
        //     result.microstep()
        // );
        result
    }

    pub fn lf_tag_add(a: &Tag, b: &Tag) -> Tag {
        if a.time() == NEVER || b.time() == NEVER {
            return Tag::never_tag();
        }
        if a.time() == FOREVER || b.time() == FOREVER {
            return Tag::forever_tag();
        }
        let result = Tag::new(a.time() + b.time(), a.microstep() + b.microstep());
        if result.microstep() < a.microstep() {
            return Tag::forever_tag();
        }
        if result.time() < a.time() && b.time() > 0 {
            return Tag::forever_tag();
        }
        if result.time() > a.time() && b.time() < 0 {
            return Tag::never_tag();
        }
        result
    }
}
