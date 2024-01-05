use crate::net_common::MsgType;
use crate::net_util::NetUtil;
/**
 * @file enclave.rs
 * @author Edward A. Lee (eal@berkeley.edu)
 * @author Soroush Bateni (soroush@utdallas.edu)
 * @author Erling Jellum (erling.r.jellum@ntnu.no)
 * @author Chadlia Jerad (chadlia.jerad@ensi-uma.tn)
 * @author Chanhee Lee (chanheel@asu.edu)
 * @author Hokeun Kim (hokeun@asu.edu)
 * @copyright (c) 2020-2024, The University of California at Berkeley
 * License in [BSD 2-clause](..)
 * @brief Declarations for runtime infrastructure (RTI) for distributed Lingua Franca programs.
 * This file extends enclave.h with RTI features that are specific to federations and are not
 * used by scheduling enclaves.
 */
use crate::rti_remote::RTIRemote;
use crate::tag;
use crate::tag::{Instant, Interval, Tag, FOREVER};
use crate::FederateInfo;
use crate::SchedulingNodeState::*;

use std::io::Write;
use std::mem;
use std::sync::{Arc, Condvar, Mutex};

const IS_IN_ZERO_DELAY_CYCLE: i32 = 1;
const IS_IN_CYCLE: i32 = 2;

/** Mode of execution of a federate. */
enum ExecutionMode {
    FAST,
    REALTIME,
}

#[derive(PartialEq, Clone, Debug)]
pub enum SchedulingNodeState {
    NotConnected, // The scheduling node has not connected.
    Granted,      // Most recent MsgType::NextEventTag has been granted.
    Pending,      // Waiting for upstream scheduling nodes.
}

/** Struct for minimum delays from upstream nodes. */
pub struct MinimumDelay {
    id: i32,        // ID of the upstream node.
    min_delay: Tag, // Minimum delay from upstream.
}

impl MinimumDelay {
    pub fn new(id: i32, min_delay: Tag) -> MinimumDelay {
        MinimumDelay { id, min_delay }
    }

    pub fn id(&self) -> i32 {
        self.id
    }

    pub fn min_delay(&self) -> &Tag {
        &self.min_delay
    }
}
/**
 * Information about the scheduling nodes coordinated by the RTI.
 * The abstract scheduling node could either be an enclave or a federate.
 * The information includes its runtime state,
 * mode of execution, and connectivity with other scheduling nodes.
 * The list of upstream and downstream scheduling nodes does not include
 * those that are connected via a "physical" connection (one
 * denoted with ~>) because those connections do not impose
 * any scheduling constraints.
 */
pub struct SchedulingNode {
    id: u16,                         // ID of this scheduling node.
    completed: Tag, // The largest logical tag completed by the federate (or NEVER if no LTC has been received).
    last_granted: Tag, // The maximum Tag that has been granted so far (or NEVER if none granted)
    last_provisionally_granted: Tag, // The maximum PTAG that has been provisionally granted (or NEVER if none granted)
    next_event: Tag, // Most recent NET received from the federate (or NEVER if none received).
    state: SchedulingNodeState, // State of the federate.
    upstream: Vec<i32>, // Array of upstream federate ids.
    upstream_delay: Vec<Interval>, // Minimum delay on connections from upstream federates.
    // Here, NEVER encodes no delay. 0LL is a microstep delay.
    num_upstream: i32,    // Size of the array of upstream federates and delays.
    downstream: Vec<i32>, // Array of downstream federate ids.
    num_downstream: i32,  // Size of the array of downstream federates.
    mode: ExecutionMode,  // FAST or REALTIME.
    min_delays: Vec<MinimumDelay>, // Array of minimum delays from upstream nodes, not including this node.
    num_min_delays: u64,           // Size of min_delays array.
    flags: i32,                    // Or of IS_IN_ZERO_DELAY_CYCLE, IS_IN_CYCLE
}

impl SchedulingNode {
    pub fn new() -> SchedulingNode {
        SchedulingNode {
            id: 0,
            completed: Tag::never_tag(),
            last_granted: Tag::never_tag(),
            last_provisionally_granted: Tag::never_tag(),
            next_event: Tag::never_tag(),
            state: SchedulingNodeState::NotConnected,
            upstream: Vec::new(),
            upstream_delay: Vec::new(),
            num_upstream: 0,
            downstream: Vec::new(),
            num_downstream: 0,
            mode: ExecutionMode::REALTIME,
            min_delays: Vec::new(),
            num_min_delays: 0,
            flags: 0,
        }
    }

    pub fn initialize_scheduling_node(&mut self, id: u16) {
        self.id = id;
        // Initialize the next event condition variable.
        // TODO: lf_cond_init(&e->next_event_condition, &rti_mutex);
    }

    pub fn id(&self) -> u16 {
        self.id
    }

    pub fn completed(&self) -> Tag {
        self.completed.clone()
    }

    pub fn last_granted(&self) -> Tag {
        self.last_granted.clone()
    }

    pub fn last_provisionally_granted(&self) -> Tag {
        self.last_provisionally_granted.clone()
    }

    pub fn next_event(&self) -> Tag {
        self.next_event.clone()
    }

    pub fn state(&self) -> SchedulingNodeState {
        self.state.clone()
    }

    pub fn upstream(&self) -> &Vec<i32> {
        &self.upstream
    }

    pub fn upstream_delay(&self) -> &Vec<Interval> {
        &self.upstream_delay
    }

    pub fn num_upstream(&self) -> i32 {
        self.num_upstream
    }

    pub fn downstream(&self) -> &Vec<i32> {
        &self.downstream
    }

    pub fn num_downstream(&self) -> i32 {
        self.num_downstream
    }

    pub fn min_delays(&mut self) -> &mut Vec<MinimumDelay> {
        &mut self.min_delays
    }

    pub fn num_min_delays(&self) -> u64 {
        self.num_min_delays
    }

    pub fn flags(&self) -> i32 {
        self.flags
    }

    pub fn set_last_granted(&mut self, tag: Tag) {
        self.last_granted = tag;
    }

    pub fn set_last_provisionally_granted(&mut self, tag: Tag) {
        self.last_provisionally_granted = tag;
    }

    pub fn set_next_event(&mut self, next_event_tag: Tag) {
        self.next_event = next_event_tag;
    }

    pub fn set_state(&mut self, state: SchedulingNodeState) {
        self.state = state;
    }

    pub fn set_upstream_id_at(&mut self, upstream_id: u16, idx: usize) {
        self.upstream.insert(idx, upstream_id as i32);
    }

    pub fn set_completed(&mut self, completed: Tag) {
        self.completed = completed.clone()
    }

    pub fn set_upstream_delay_at(&mut self, upstream_delay: tag::Interval, idx: usize) {
        self.upstream_delay.insert(idx, upstream_delay);
    }

    pub fn set_num_upstream(&mut self, num_upstream: i32) {
        self.num_upstream = num_upstream;
    }

    pub fn set_downstream_id_at(&mut self, downstream_id: u16, idx: usize) {
        self.downstream.insert(idx, downstream_id as i32);
    }

    pub fn set_num_downstream(&mut self, num_downstream: i32) {
        self.num_downstream = num_downstream;
    }

    pub fn set_num_min_delays(&mut self, num_min_delays: u64) {
        self.num_min_delays = num_min_delays;
    }

    pub fn set_flags(&mut self, flags: i32) {
        self.flags = flags;
    }

    pub fn update_scheduling_node_next_event_tag_locked(
        _f_rti: Arc<Mutex<RTIRemote>>,
        fed_id: u16,
        next_event_tag: Tag,
        start_time: Instant,
        sent_start_time: Arc<(Mutex<bool>, Condvar)>,
    ) {
        let num_upstream;
        let number_of_scheduling_nodes;
        {
            let mut locked_rti = _f_rti.lock().unwrap();
            number_of_scheduling_nodes = locked_rti.base().number_of_scheduling_nodes();
            let idx: usize = fed_id.into();
            let fed = &mut locked_rti.base().scheduling_nodes()[idx];
            let e = fed.enclave();
            e.set_next_event(next_event_tag.clone());
            num_upstream = e.num_upstream();
        }
        println!(
            "RTI: Updated the recorded next event tag for federate/enclave {} to ({},{})",
            fed_id,
            next_event_tag.time() - start_time,
            next_event_tag.microstep()
        );

        // Check to see whether we can reply now with a tag advance grant.
        // If the enclave has no upstream enclaves, then it does not wait for
        // nor expect a reply. It just proceeds to advance time.
        if num_upstream > 0 {
            Self::notify_advance_grant_if_safe(
                _f_rti.clone(),
                fed_id,
                number_of_scheduling_nodes,
                start_time,
                sent_start_time.clone(),
            );
        }
        // Check downstream enclaves to see whether they should now be granted a TAG.
        // To handle cycles, need to create a boolean array to keep
        // track of which upstream enclaves have been visited.
        let mut visited = vec![false as bool; number_of_scheduling_nodes as usize]; // Initializes to 0.
        Self::notify_downstream_advance_grant_if_safe(
            _f_rti.clone(),
            fed_id,
            number_of_scheduling_nodes,
            start_time,
            &mut visited,
            sent_start_time,
        );
    }

    fn notify_advance_grant_if_safe(
        _f_rti: Arc<Mutex<RTIRemote>>,
        fed_id: u16,
        number_of_enclaves: i32,
        start_time: Instant,
        sent_start_time: Arc<(Mutex<bool>, Condvar)>,
    ) {
        let grant =
            Self::tag_advance_grant_if_safe(_f_rti.clone(), fed_id, number_of_enclaves, start_time);
        if Tag::lf_tag_compare(&grant.tag(), &Tag::never_tag()) != 0 {
            if grant.is_provisional() {
                Self::notify_provisional_tag_advance_grant(
                    _f_rti,
                    fed_id,
                    number_of_enclaves,
                    grant.tag(),
                    start_time,
                    sent_start_time,
                );
            } else {
                Self::notify_tag_advance_grant(
                    _f_rti,
                    fed_id,
                    grant.tag(),
                    start_time,
                    sent_start_time,
                );
            }
        }
    }

    fn tag_advance_grant_if_safe(
        _f_rti: Arc<Mutex<RTIRemote>>,
        fed_id: u16,
        number_of_enclaves: i32,
        start_time: Instant,
    ) -> TagAdvanceGrant {
        let mut result = TagAdvanceGrant::new(Tag::never_tag(), false);

        // Find the earliest LTC of upstream enclaves (M).
        {
            let mut min_upstream_completed = Tag::forever_tag();
            let mut locked_rti = _f_rti.lock().unwrap();
            let scheduling_nodes = locked_rti.base().scheduling_nodes();
            let idx: usize = fed_id.into();
            let e = scheduling_nodes[idx].e();
            let upstreams = e.upstream();
            let upstream_delay = e.upstream_delay();
            for j in 0..upstreams.len() {
                let delay = upstream_delay[j];
                // FIXME: Replace "as usize" properly.
                let upstream = &scheduling_nodes[upstreams[j] as usize].e();
                // Ignore this enclave if it no longer connected.
                if upstream.state() == SchedulingNodeState::NotConnected {
                    continue;
                }

                // Adjust by the "after" delay.
                // Note that "no delay" is encoded as NEVER,
                // whereas one microstep delay is encoded as 0LL.
                let candidate = Tag::lf_delay_strict(&upstream.completed(), delay);

                if Tag::lf_tag_compare(&candidate, &min_upstream_completed) < 0 {
                    min_upstream_completed = candidate.clone();
                }
            }
            println!(
                "Minimum upstream LTC for federate/enclave {} is ({},{}) (adjusted by after delay).",
                e.id(),
                // FIXME: Check the below calculation
                min_upstream_completed.time(), // - start_time,
                min_upstream_completed.microstep()
            );
            if Tag::lf_tag_compare(&min_upstream_completed, &e.last_granted()) > 0
                && Tag::lf_tag_compare(&min_upstream_completed, &e.next_event()) >= 0
            // The enclave has to advance its tag
            {
                result.set_tag(min_upstream_completed);
                return result;
            }
        }

        // Can't make progress based only on upstream LTCs.
        // If all (transitive) upstream enclaves of the enclave
        // have earliest event tags such that the
        // enclave can now advance its tag, then send it a TAG message.
        // Find the tag of the earliest event that may be later received from an upstream enclave
        // or federate (which includes any after delays on the connections).
        let t_d =
            Self::earliest_future_incoming_message_tag(_f_rti.clone(), fed_id as u16, start_time);

        println!(
            "RTI: Earliest next event upstream of node {} has tag ({},{}).",
            fed_id,
            t_d.time() - start_time,
            t_d.microstep()
        );

        // Given an EIMT (earliest incoming message tag) there are these possible scenarios:
        //  1) The EIMT is greater than the NET we want to advance to. Grant a TAG.
        //  2) The EIMT is equal to the NET and the federate is part of a zero-delay cycle (ZDC).
        //  3) The EIMT is equal to the NET and the federate is not part of a ZDC.
        //  4) The EIMT is less than the NET
        // In (1) we can give a TAG to NET. In (2) we can give a PTAG.
        // In (3) and (4), we wait for further updates from upstream federates.
        let next_event;
        let last_provisionally_granted;
        let last_granted;
        {
            let mut locked_rti = _f_rti.lock().unwrap();
            let scheduling_nodes = locked_rti.base().scheduling_nodes();
            let idx: usize = fed_id.into();
            let e = scheduling_nodes[idx].e();
            next_event = e.next_event();
            last_provisionally_granted = e.last_provisionally_granted();
            last_granted = e.last_granted();
        }
        if
        // Scenario (1) above
        Tag::lf_tag_compare(&t_d, &next_event) > 0                      // EIMT greater than NET
            && Tag::lf_tag_compare(&t_d, &last_provisionally_granted) >= 0  // The grant is not redundant
                                                                        // (equal is important to override any previous
                                                                        // PTAGs).
            && Tag::lf_tag_compare(&t_d, &last_granted) > 0
        // The grant is not redundant.
        {
            // No upstream node can send events that will be received with a tag less than or equal to
            // e->next_event, so it is safe to send a TAG.
            println!("RTI: Earliest upstream message time for fed/encl {} is ({},{})(adjusted by after delay). Granting tag advance (TAG) for ({},{})",
                    fed_id,
                    t_d.time() - start_time, t_d.microstep(),
                    next_event.time(), // TODO: - start_time,
                    next_event.microstep());
            result.set_tag(next_event);
        } else if
        // Scenario (2) or (3) above
        Tag::lf_tag_compare(&t_d, &next_event) == 0                     // EIMT equal to NET
            && Self::is_in_zero_delay_cycle(_f_rti.clone(), fed_id)                                // The node is part of a ZDC
            && Tag::lf_tag_compare(&t_d, &last_provisionally_granted) > 0   // The grant is not redundant
            && Tag::lf_tag_compare(&t_d, &last_granted) > 0
        // The grant is not redundant.
        {
            // Some upstream node may send an event that has the same tag as this node's next event,
            // so we can only grant a PTAG.
            println!("RTI: Earliest upstream message time for fed/encl {} is ({},{})(adjusted by after delay). Granting provisional tag advance (PTAG) for ({},{})",
                fed_id,
                t_d.time() - start_time, t_d.microstep(),
                next_event.time() - start_time,
                next_event.microstep());
            result.set_tag(next_event);
            result.set_provisional(true);
        }
        result
    }

    fn is_in_zero_delay_cycle(_f_rti: Arc<Mutex<RTIRemote>>, fed_id: u16) -> bool {
        Self::update_min_delays_upstream(_f_rti.clone(), fed_id);
        let flags;
        {
            let mut locked_rti = _f_rti.lock().unwrap();
            let scheduling_nodes = locked_rti.base().scheduling_nodes();
            let idx: usize = fed_id.into();
            let node = scheduling_nodes[idx].e();
            flags = node.flags()
        }
        (flags & IS_IN_ZERO_DELAY_CYCLE) != 0
    }

    fn transitive_next_event(
        enclaves: &Vec<FederateInfo>,
        e: &SchedulingNode,
        candidate: Tag,
        visited: &mut Vec<bool>,
        start_time: Instant,
    ) -> Tag {
        // FIXME: Replace "as usize" properly.
        if visited[e.id() as usize] || e.state() == SchedulingNodeState::NotConnected {
            // SchedulingNode has stopped executing or we have visited it before.
            // No point in checking upstream enclaves.
            return candidate.clone();
        }

        // FIXME: Replace "as usize" properly.
        visited[e.id() as usize] = true;
        let mut result = e.next_event();

        // If the candidate is less than this enclave's next_event, use the candidate.
        if Tag::lf_tag_compare(&candidate, &result) < 0 {
            result = candidate.clone();
        }

        // The result cannot be earlier than the start time.
        if result.time() < start_time {
            // Earliest next event cannot be before the start time.
            result = Tag::new(start_time, 0);
        }

        // Check upstream enclaves to see whether any of them might send
        // an event that would result in an earlier next event.
        for i in 0..e.upstream().len() {
            // FIXME: Replace "as usize" properly.
            let upstream = enclaves[e.upstream()[i] as usize].e();
            let mut upstream_result = Self::transitive_next_event(
                enclaves,
                upstream,
                result.clone(),
                visited,
                start_time,
            );

            // Add the "after" delay of the connection to the result.
            upstream_result = Tag::lf_delay_tag(&upstream_result, e.upstream_delay()[i]);

            // If the adjusted event time is less than the result so far, update the result.
            if Tag::lf_tag_compare(&upstream_result, &result) < 0 {
                result = upstream_result;
            }
        }
        let completed = e.completed();
        if Tag::lf_tag_compare(&result, &completed) < 0 {
            result = completed;
        }

        result
    }

    fn notify_tag_advance_grant(
        _f_rti: Arc<Mutex<RTIRemote>>,
        fed_id: u16,
        tag: Tag,
        start_time: Instant,
        sent_start_time: Arc<(Mutex<bool>, Condvar)>,
    ) {
        {
            let mut locked_rti = _f_rti.lock().unwrap();
            let enclaves = locked_rti.base().scheduling_nodes();
            let idx: usize = fed_id.into();
            let fed: &FederateInfo = &enclaves[idx];
            let e = fed.e();
            if e.state() == SchedulingNodeState::NotConnected
                || Tag::lf_tag_compare(&tag, &e.last_granted()) <= 0
                || Tag::lf_tag_compare(&tag, &e.last_provisionally_granted()) <= 0
            {
                return;
            }
            // Need to make sure that the destination federate's thread has already
            // sent the starting MSG_TYPE_TIMESTAMP message.
            while e.state() == SchedulingNodeState::Pending {
                // Need to wait here.
                let (lock, condvar) = &*sent_start_time;
                let mut notified = lock.lock().unwrap();
                while !*notified {
                    notified = condvar.wait(notified).unwrap();
                }
            }
        }
        let message_length = 1 + mem::size_of::<i64>() + mem::size_of::<u32>();
        // FIXME: Replace "as usize" properly.
        let mut buffer = vec![0 as u8; message_length as usize];
        buffer[0] = MsgType::TagAdvanceGrant.to_byte();
        NetUtil::encode_int64(tag.time(), &mut buffer, 1);
        // FIXME: Replace "as i32" properly.
        NetUtil::encode_int32(
            tag.microstep() as i32,
            &mut buffer,
            1 + mem::size_of::<i64>(),
        );

        // This function is called in notify_advance_grant_if_safe(), which is a long
        // function. During this call, the socket might close, causing the following write_to_socket
        // to fail. Consider a failure here a soft failure and update the federate's status.
        let mut error_occurred = false;
        {
            let mut locked_rti = _f_rti.lock().unwrap();
            let scheduling_nodes = locked_rti.base().scheduling_nodes();
            // FIXME: Replace "as usize" properly.
            let fed: &FederateInfo = &scheduling_nodes[fed_id as usize];
            let e = fed.e();
            let mut stream = fed.stream().as_ref().unwrap();
            match stream.write(&buffer) {
                Ok(bytes_written) => {
                    if bytes_written < message_length {
                        println!(
                            "RTI failed to send tag advance grant to federate {}.",
                            e.id()
                        );
                    }
                }
                Err(_err) => {
                    error_occurred = true;
                }
            }
        }
        {
            let mut locked_rti = _f_rti.lock().unwrap();
            // FIXME: Replace "as usize" properly.
            let mut_fed: &mut FederateInfo =
                &mut locked_rti.base().scheduling_nodes()[fed_id as usize];
            let enclave = mut_fed.enclave();
            if error_occurred {
                enclave.set_state(SchedulingNodeState::NotConnected);
                // FIXME: We need better error handling, but don't stop other execution here.
            } else {
                enclave.set_last_granted(tag.clone());
                println!(
                    "RTI sent to federate {} the Tag Advance Grant (TAG) ({},{}).",
                    enclave.id(),
                    tag.time() - start_time,
                    tag.microstep()
                );
            }
        }
    }

    fn notify_provisional_tag_advance_grant(
        _f_rti: Arc<Mutex<RTIRemote>>,
        fed_id: u16,
        number_of_enclaves: i32,
        tag: Tag,
        start_time: Instant,
        sent_start_time: Arc<(Mutex<bool>, Condvar)>,
    ) {
        {
            let mut locked_rti = _f_rti.lock().unwrap();
            let enclaves = locked_rti.base().scheduling_nodes();
            let idx: usize = fed_id.into();
            let fed: &FederateInfo = &enclaves[idx];
            let e = fed.e();
            if e.state() == SchedulingNodeState::NotConnected
                || Tag::lf_tag_compare(&tag, &e.last_granted()) <= 0
                || Tag::lf_tag_compare(&tag, &e.last_provisionally_granted()) <= 0
            {
                return;
            }
            // Need to make sure that the destination federate's thread has already
            // sent the starting MSG_TYPE_TIMESTAMP message.
            while e.state() == SchedulingNodeState::Pending {
                // Need to wait here.
                let (lock, condvar) = &*sent_start_time;
                let mut notified = lock.lock().unwrap();
                while !*notified {
                    notified = condvar.wait(notified).unwrap();
                }
            }
        }
        let message_length = 1 + mem::size_of::<i64>() + mem::size_of::<u32>();
        // FIXME: Replace "as usize" properly.
        let mut buffer = vec![0 as u8; message_length as usize];
        buffer[0] = MsgType::PropositionalTagAdvanceGrant.to_byte();
        NetUtil::encode_int64(tag.time(), &mut buffer, 1);
        NetUtil::encode_int32(
            tag.microstep().try_into().unwrap(),
            &mut buffer,
            1 + mem::size_of::<i64>(),
        );

        // This function is called in notify_advance_grant_if_safe(), which is a long
        // function. During this call, the socket might close, causing the following write_to_socket
        // to fail. Consider a failure here a soft failure and update the federate's status.
        let mut error_occurred = false;
        {
            let mut locked_rti = _f_rti.lock().unwrap();
            let enclaves = locked_rti.base().scheduling_nodes();
            // FIXME: Replace "as usize" properly.
            let fed: &FederateInfo = &enclaves[fed_id as usize];
            let e = fed.e();
            let mut stream = fed.stream().as_ref().unwrap();
            match stream.write(&buffer) {
                Ok(bytes_written) => {
                    if bytes_written < message_length {
                        println!(
                            "RTI failed to send tag advance grant to federate {}.",
                            e.id()
                        );
                        return;
                    }
                }
                Err(_err) => {
                    error_occurred = true;
                }
            }
        }
        {
            let mut locked_rti = _f_rti.lock().unwrap();
            // FIXME: Replace "as usize" properly.
            let mut_fed: &mut FederateInfo =
                &mut locked_rti.base().scheduling_nodes()[fed_id as usize];
            let enclave = mut_fed.enclave();
            if error_occurred {
                enclave.set_state(SchedulingNodeState::NotConnected);
                // FIXME: We need better error handling, but don't stop other execution here.
            }

            enclave.set_last_provisionally_granted(tag.clone());
            println!(
                "RTI sent to federate {} the Provisional Tag Advance Grant (PTAG) ({},{}).",
                enclave.id(),
                tag.time() - start_time,
                tag.microstep()
            );
        }

        // Send PTAG to all upstream federates, if they have not had
        // a later or equal PTAG or TAG sent previously and if their transitive
        // NET is greater than or equal to the tag.
        // NOTE: This could later be replaced with a TNET mechanism once
        // we have an available encoding of causality interfaces.
        // That might be more efficient.
        // NOTE: This is not needed for enclaves because zero-delay loops are prohibited.
        // It's only needed for federates, which is why this is implemented here.
        let num_upstream;
        {
            let mut locked_rti = _f_rti.lock().unwrap();
            let enclaves = locked_rti.base().scheduling_nodes();
            let idx: usize = fed_id.into();
            let fed: &FederateInfo = &enclaves[idx];
            let e = fed.e();
            num_upstream = e.num_upstream();
        }
        for j in 0..num_upstream {
            let e_id;
            let earlist;
            {
                let mut locked_rti = _f_rti.lock().unwrap();
                let enclaves = locked_rti.base().scheduling_nodes();
                let idx: usize = fed_id.into();
                let fed: &FederateInfo = &enclaves[idx];
                // FIXME: Replace "as usize" properly.
                e_id = fed.e().upstream()[j as usize];
                // FIXME: Replace "as usize" properly.
                let upstream: &FederateInfo = &enclaves[e_id as usize];

                // Ignore this federate if it has resigned.
                if upstream.e().state() == NotConnected {
                    continue;
                }

                // FIXME: Replace "as u16" properly.
                earlist = Self::earliest_future_incoming_message_tag(
                    _f_rti.clone(),
                    e_id as u16,
                    start_time,
                );
            }
            // If these tags are equal, then a TAG or PTAG should have already been granted,
            // in which case, another will not be sent. But it may not have been already granted.
            if Tag::lf_tag_compare(&earlist, &tag) >= 0 {
                Self::notify_provisional_tag_advance_grant(
                    _f_rti.clone(),
                    // FIXME: Handle unwrap properly.
                    e_id.try_into().unwrap(),
                    number_of_enclaves,
                    tag.clone(),
                    start_time,
                    sent_start_time.clone(),
                );
            }
        }
    }

    fn earliest_future_incoming_message_tag(
        _f_rti: Arc<Mutex<RTIRemote>>,
        fed_id: u16,
        start_time: Instant,
    ) -> Tag {
        let num_min_delays;
        {
            let mut locked_rti = _f_rti.lock().unwrap();
            let enclaves = locked_rti.base().scheduling_nodes();
            let idx: usize = fed_id.into();
            let fed: &FederateInfo = &enclaves[idx];
            let e = fed.e();
            num_min_delays = e.num_min_delays();
        }
        // First, we need to find the shortest path (minimum delay) path to each upstream node
        // and then find the minimum of the node's recorded NET plus the minimum path delay.
        // Update the shortest paths, if necessary.
        Self::update_min_delays_upstream(_f_rti.clone(), fed_id);

        // Next, find the tag of the earliest possible incoming message from upstream enclaves or
        // federates, which will be the smallest upstream NET plus the least delay.
        // This could be NEVER_TAG if the RTI has not seen a NET from some upstream node.
        let mut t_d = Tag::forever_tag();
        for i in 0..num_min_delays {
            let upstream_id;
            {
                let mut locked_rti = _f_rti.lock().unwrap();
                let enclaves = locked_rti.base().scheduling_nodes();
                let idx: usize = fed_id.into();
                let fed: &FederateInfo = &enclaves[idx];
                let e = fed.e();
                // FIXME: Handle "as usize" properly.
                upstream_id = e.min_delays[i as usize].id() as usize;
            }
            let upstream_next_event;
            {
                // Node e->min_delays[i].id is upstream of e with min delay e->min_delays[i].min_delay.
                let mut locked_rti = _f_rti.lock().unwrap();
                let enclaves = locked_rti.base().scheduling_nodes();
                let fed: &mut FederateInfo = &mut enclaves[upstream_id];
                let upstream = fed.enclave();
                // If we haven't heard from the upstream node, then assume it can send an event at the start time.
                upstream_next_event = upstream.next_event();
                if Tag::lf_tag_compare(&upstream_next_event, &Tag::never_tag()) == 0 {
                    let start_tag = Tag::new(start_time, 0);
                    upstream.set_next_event(start_tag);
                }
            }
            let min_delay;
            let earliest_tag_from_upstream;
            {
                let mut locked_rti = _f_rti.lock().unwrap();
                let enclaves = locked_rti.base().scheduling_nodes();
                let idx: usize = fed_id.into();
                let fed: &mut FederateInfo = &mut enclaves[idx];
                let e = fed.enclave();
                // FIXME: Handle "as usize" properly.
                min_delay = e.min_delays()[i as usize].min_delay();
                earliest_tag_from_upstream = Tag::lf_tag_add(&upstream_next_event, &min_delay);
                println!("RTI: Earliest next event upstream of fed/encl {} at fed/encl {} has tag ({},{}).",
                    fed_id,
                    upstream_id,
                    earliest_tag_from_upstream.time() - start_time, earliest_tag_from_upstream.microstep());
            }
            if Tag::lf_tag_compare(&earliest_tag_from_upstream, &t_d) < 0 {
                t_d = earliest_tag_from_upstream.clone();
            }
        }
        t_d
    }

    fn update_min_delays_upstream(_f_rti: Arc<Mutex<RTIRemote>>, node_idx: u16) {
        let num_min_delays;
        let number_of_scheduling_nodes;
        {
            let mut locked_rti = _f_rti.lock().unwrap();
            let scheduling_nodes = locked_rti.base().scheduling_nodes();
            let idx: usize = node_idx.into();
            num_min_delays = scheduling_nodes[idx].e().num_min_delays();
            number_of_scheduling_nodes = locked_rti.base().number_of_scheduling_nodes();
        }
        // Check whether cached result is valid.
        if num_min_delays == 0 {
            // This is not Dijkstra's algorithm, but rather one optimized for sparse upstream nodes.
            // There must be a name for this algorithm.

            // Array of results on the stack:
            let mut path_delays = Vec::new();
            // This will be the number of non-FOREVER entries put into path_delays.
            let mut count: u64 = 0;

            for _i in 0..number_of_scheduling_nodes {
                path_delays.push(Tag::forever_tag());
            }
            // FIXME:: Handle "as i32" properly.
            Self::_update_min_delays_upstream(
                _f_rti.clone(),
                node_idx as i32,
                -1,
                &mut path_delays,
                &mut count,
            );

            // Put the results onto the node's struct.
            {
                let mut locked_rti = _f_rti.lock().unwrap();
                let scheduling_nodes = locked_rti.base().scheduling_nodes();
                let idx: usize = node_idx.into();
                let node = scheduling_nodes[idx].enclave();
                node.set_num_min_delays(count);
                println!(
                    "++++ Node {}(is in ZDC: {}\n",
                    node_idx,
                    node.flags() & IS_IN_ZERO_DELAY_CYCLE
                );

                let mut k = 0;
                for i in 0..number_of_scheduling_nodes {
                    // FIXME: Handle "as usize" properly.
                    if Tag::lf_tag_compare(&path_delays[i as usize], &Tag::forever_tag()) < 0 {
                        // Node i is upstream.
                        if k >= count {
                            println!(
                                "Internal error! Count of upstream nodes {} for node {} is wrong!",
                                count, i
                            );
                            std::process::exit(1);
                        }
                        // FIXME: Handle "as usize" properly.
                        let min_delay = MinimumDelay::new(i, path_delays[i as usize].clone());
                        let min_delays = node.min_delays();
                        // FIXME: Handle unwrap() properly.
                        min_delays.insert(k.try_into().unwrap(), min_delay);
                        k = k + 1;
                        // N^2 debug statement could be a problem with large benchmarks.
                        // println!("++++    Node {} is upstream with delay ({},{})", i, path_delays[i].time(), path_delays[i].microstep());
                    }
                }
            }
        }
    }

    // Local function used recursively to find minimum delays upstream.
    // Return in count the number of non-FOREVER_TAG entries in path_delays[].
    fn _update_min_delays_upstream(
        _f_rti: Arc<Mutex<RTIRemote>>,
        end_idx: i32,
        mut intermediate_idx: i32,
        path_delays: &mut Vec<Tag>,
        count: &mut u64,
    ) {
        // On first call, intermediate will be NULL, so the path delay is initialized to zero.
        let mut delay_from_intermediate_so_far = Tag::zero_tag();
        if intermediate_idx < 0 {
            intermediate_idx = end_idx;
        } else {
            // Not the first call, so intermediate is upstream of end.
            // FIXME: Handle "as usize" properly.
            delay_from_intermediate_so_far = path_delays[intermediate_idx as usize].clone();
        }
        {
            let mut locked_rti = _f_rti.lock().unwrap();
            let fed: &FederateInfo =
                &locked_rti.base().scheduling_nodes()[intermediate_idx as usize];
            let intermediate = fed.e();
            if intermediate.state() == SchedulingNodeState::NotConnected {
                // Enclave or federate is not connected.
                // No point in checking upstream scheduling_nodes.
                return;
            }
        }
        // Check nodes upstream of intermediate (or end on first call).
        // NOTE: It would be better to iterate through these sorted by minimum delay,
        // but for most programs, the gain might be negligible since there are relatively few
        // upstream nodes.
        let num_upstream;
        {
            let mut locked_rti = _f_rti.lock().unwrap();
            let fed: &FederateInfo =
                &locked_rti.base().scheduling_nodes()[intermediate_idx as usize];
            let e = fed.e();
            num_upstream = e.num_upstream();
        }
        for i in 0..num_upstream {
            let upstream_idx;
            let upstream_delay;
            {
                let mut locked_rti = _f_rti.lock().unwrap();
                let scheduling_nodes = locked_rti.base().scheduling_nodes();
                // FIXME: Handle "as usize" properly.
                let e = scheduling_nodes[intermediate_idx as usize].e();
                // FIXME: Handle "as usize" properly.
                upstream_idx = e.upstream[i as usize];
                // FIXME: Handle "as usize" properly.
                upstream_delay = e.upstream_delay[i as usize];
            }
            // Add connection delay to path delay so far.
            let path_delay = Tag::lf_delay_tag(&delay_from_intermediate_so_far, upstream_delay);
            // If the path delay is less than the so-far recorded path delay from upstream, update upstream.
            // FIXME: Handle "as usize" properly.
            if Tag::lf_tag_compare(&path_delay, &path_delays[upstream_idx as usize]) < 0 {
                // FIXME: Handle "as usize" properly.
                if path_delays[upstream_idx as usize].time() == FOREVER {
                    // Found a finite path.
                    *count = *count + 1;
                }
                // FIXME: Handle "as usize" properly.
                path_delays.insert(upstream_idx as usize, path_delay.clone());
                // Since the path delay to upstream has changed, recursively update those upstream of it.
                // Do not do this, however, if the upstream node is the end node because this means we have
                // completed a cycle.
                if end_idx != upstream_idx {
                    Self::_update_min_delays_upstream(
                        _f_rti.clone(),
                        end_idx,
                        intermediate_idx,
                        path_delays,
                        count,
                    );
                } else {
                    let mut locked_rti = _f_rti.lock().unwrap();
                    let scheduling_nodes = locked_rti.base().scheduling_nodes();
                    // FIXME: Handle "as usize" properly.
                    let end: &mut SchedulingNode = scheduling_nodes[end_idx as usize].enclave();
                    // Found a cycle.
                    end.set_flags(end.flags() | IS_IN_CYCLE);
                    // Is it a zero-delay cycle?
                    if Tag::lf_tag_compare(&path_delay, &Tag::zero_tag()) == 0
                        && upstream_delay < Some(0)
                    {
                        end.set_flags(end.flags() | IS_IN_ZERO_DELAY_CYCLE);
                    } else {
                        // Clear the flag.
                        end.set_flags(end.flags() & !IS_IN_ZERO_DELAY_CYCLE);
                    }
                }
            }
        }
    }

    pub fn notify_downstream_advance_grant_if_safe(
        _f_rti: Arc<Mutex<RTIRemote>>,
        fed_id: u16,
        number_of_enclaves: i32,
        start_time: Instant,
        visited: &mut Vec<bool>,
        sent_start_time: Arc<(Mutex<bool>, Condvar)>,
    ) {
        // FIXME: Replace "as usize" properly.
        visited[fed_id as usize] = true;
        let num_downstream;
        {
            let mut locked_rti = _f_rti.lock().unwrap();
            let idx: usize = fed_id.into();
            let fed: &FederateInfo = &locked_rti.base().scheduling_nodes()[idx];
            let e = fed.e();
            num_downstream = e.num_downstream();
        }
        for i in 0..num_downstream {
            let e_id;
            {
                let mut locked_rti = _f_rti.lock().unwrap();
                let enclaves = locked_rti.base().scheduling_nodes();
                let idx: usize = fed_id.into();
                let fed: &FederateInfo = &enclaves[idx];
                let downstreams = fed.e().downstream();
                // FIXME: Replace "as u16" properly.
                e_id = downstreams[i as usize] as u16;
                // FIXME: Replace "as usize" properly.
                if visited[e_id as usize] {
                    continue;
                }
            }
            Self::notify_advance_grant_if_safe(
                _f_rti.clone(),
                e_id,
                number_of_enclaves,
                start_time,
                sent_start_time.clone(),
            );
            Self::notify_downstream_advance_grant_if_safe(
                _f_rti.clone(),
                e_id,
                number_of_enclaves,
                start_time,
                visited,
                sent_start_time.clone(),
            );
        }
    }

    pub fn logical_tag_complete(
        _f_rti: Arc<Mutex<RTIRemote>>,
        fed_id: u16,
        number_of_enclaves: i32,
        start_time: Instant,
        sent_start_time: Arc<(Mutex<bool>, Condvar)>,
        completed: Tag,
    ) {
        // FIXME: Consolidate this message with NET to get NMR (Next Message Request).
        // Careful with handling startup and shutdown.
        {
            let mut locked_rti = _f_rti.lock().unwrap();
            let idx: usize = fed_id.into();
            let fed: &mut FederateInfo = &mut locked_rti.base().scheduling_nodes()[idx];
            let enclave = fed.enclave();
            enclave.set_completed(completed);

            println!(
                "RTI received from federate/enclave {} the Logical Tag Complete (LTC) ({},{}).",
                enclave.id(),
                enclave.completed().time() - start_time,
                enclave.completed().microstep()
            );
        }

        // Check downstream enclaves to see whether they should now be granted a TAG.
        let num_downstream;
        {
            let mut locked_rti = _f_rti.lock().unwrap();
            let idx: usize = fed_id.into();
            let fed: &FederateInfo = &locked_rti.base().scheduling_nodes()[idx];
            let e = fed.e();
            num_downstream = e.num_downstream();
        }
        for i in 0..num_downstream {
            let e_id;
            {
                let mut locked_rti = _f_rti.lock().unwrap();
                let idx: usize = fed_id.into();
                let fed: &FederateInfo = &locked_rti.base().scheduling_nodes()[idx];
                let downstreams = fed.e().downstream();
                // FIXME: Replace "as u16" properly.
                e_id = downstreams[i as usize] as u16;
            }
            // Notify downstream enclave if appropriate.
            Self::notify_advance_grant_if_safe(
                _f_rti.clone(),
                e_id,
                number_of_enclaves,
                start_time,
                sent_start_time.clone(),
            );
            let mut visited = vec![false as bool; number_of_enclaves as usize]; // Initializes to 0.
                                                                                // Notify enclaves downstream of downstream if appropriate.
            Self::notify_downstream_advance_grant_if_safe(
                _f_rti.clone(),
                e_id,
                number_of_enclaves,
                start_time,
                &mut visited,
                sent_start_time.clone(),
            );
        }
    }
}

pub struct RTICommon {
    // The scheduling nodes.
    scheduling_nodes: Vec<FederateInfo>,

    // Number of scheduling nodes
    number_of_scheduling_nodes: i32,

    // RTI's decided stop tag for the scheduling nodes
    max_stop_tag: Tag,

    // Number of scheduling nodes handling stop
    num_scheduling_nodes_handling_stop: i32,

    // Boolean indicating that tracing is enabled.
    tracing_enabled: bool,
    // Pointer to a tracing object
    // TODO: trace_t* trace;

    // The RTI mutex for making thread-safe access to the shared state.
    // TODO: lf_mutex_t* mutex;
}

impl RTICommon {
    pub fn new() -> RTICommon {
        RTICommon {
            scheduling_nodes: Vec::new(),
            number_of_scheduling_nodes: 0,
            max_stop_tag: Tag::never_tag(),
            num_scheduling_nodes_handling_stop: 0,
            tracing_enabled: false,
        }
    }

    pub fn scheduling_nodes(&mut self) -> &mut Vec<FederateInfo> {
        &mut self.scheduling_nodes
    }

    pub fn number_of_scheduling_nodes(&self) -> i32 {
        self.number_of_scheduling_nodes
    }

    pub fn max_stop_tag(&self) -> Tag {
        self.max_stop_tag.clone()
    }

    pub fn num_scheduling_nodes_handling_stop(&self) -> i32 {
        self.num_scheduling_nodes_handling_stop
    }

    pub fn set_max_stop_tag(&mut self, max_stop_tag: Tag) {
        self.max_stop_tag = max_stop_tag.clone();
    }

    pub fn set_number_of_scheduling_nodes(&mut self, number_of_scheduling_nodes: i32) {
        self.number_of_scheduling_nodes = number_of_scheduling_nodes;
    }

    pub fn set_num_scheduling_nodes_handling_stop(
        &mut self,
        num_scheduling_nodes_handling_stop: i32,
    ) {
        self.num_scheduling_nodes_handling_stop = num_scheduling_nodes_handling_stop;
    }
}

struct TagAdvanceGrant {
    tag: Tag,             // NEVER if there is no tag advance grant.
    is_provisional: bool, // True for PTAG, false for TAG.
}

impl TagAdvanceGrant {
    pub fn new(tag: Tag, is_provisional: bool) -> TagAdvanceGrant {
        TagAdvanceGrant {
            tag,
            is_provisional,
        }
    }

    pub fn tag(&self) -> Tag {
        self.tag.clone()
    }

    pub fn is_provisional(&self) -> bool {
        self.is_provisional
    }

    pub fn set_tag(&mut self, tag: Tag) {
        self.tag = tag.clone();
    }

    pub fn set_provisional(&mut self, is_provisional: bool) {
        self.is_provisional = is_provisional;
    }
}
