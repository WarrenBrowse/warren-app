//! The proc connector messages the Linux resolver listens to, so it learns
//! of every process that starts, replaces its program or exits without
//! walking `/proc` for each new flow. The kernel sends each event from the
//! fork, exec or exit itself, so an event is queued on the listening socket
//! before the process it names can open a socket.
//!
//! An event can also be lost without the socket reporting it (the kernel
//! allocates it without waiting and drops it when that fails). Each CPU
//! numbers the events it sends, so a gap in a CPU's numbers is a lost event.

use std::collections::HashMap;

const NLMSG_HDR_LEN: usize = 16;
const NLMSG_DONE: u16 = 3;
/// `struct cn_msg`: the callback id, a sequence, an ack, a length, flags.
const CN_MSG_LEN: usize = 20;
/// `CN_IDX_PROC`, which is also the multicast group events are sent to.
pub const CN_IDX_PROC: u32 = 1;
const CN_VAL_PROC: u32 = 1;
const PROC_CN_MCAST_LISTEN: u32 = 1;
const PROC_CN_MCAST_IGNORE: u32 = 2;

const PROC_EVENT_NONE: u32 = 0;
const PROC_EVENT_FORK: u32 = 1;
const PROC_EVENT_EXEC: u32 = 2;
const PROC_EVENT_EXIT: u32 = 0x8000_0000;
/// `struct proc_event`: `what`, `cpu`, an 8-byte timestamp, then the event.
const EVENT_DATA_AT: usize = 16;

/// One proc connector message.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(test, derive(Debug))]
pub struct Message {
    /// The CPU that sent it, and its number among that CPU's events.
    pub cpu: u32,
    pub sequence: u32,
    pub event: Event,
}

/// What a proc connector message says, as far as process tracking goes.
// A pid ties traffic to a program: rendered in tests only.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(test, derive(Debug))]
pub enum Event {
    /// The process `tgid` started, replaced its program or exited: whatever
    /// it runs now has to be read again.
    Changed(u32),
    /// An event about a thread, an id or a name, which changes no program.
    Other,
    /// The kernel's answer to a listen request whose ack field was `ack - 1`.
    /// It is not numbered like the events.
    Acknowledged { ack: u32, error: u32 },
}

/// The request that subscribes the sending socket to process events. The
/// kernel acknowledges it with `ack + 1`.
pub fn listen_request(sequence: u32, ack: u32) -> Vec<u8> {
    request(sequence, ack, PROC_CN_MCAST_LISTEN)
}

/// The request that unsubscribes the sending socket. Kernels before 6.6
/// count listeners in one global counter and only a request lowers it, so a
/// subscribed socket sends it before closing.
pub fn ignore_request(sequence: u32) -> Vec<u8> {
    request(sequence, 0, PROC_CN_MCAST_IGNORE)
}

fn request(sequence: u32, ack: u32, operation: u32) -> Vec<u8> {
    let length = NLMSG_HDR_LEN + CN_MSG_LEN + 4;
    let mut message = Vec::with_capacity(length);
    message.extend_from_slice(&(length as u32).to_ne_bytes());
    message.extend_from_slice(&NLMSG_DONE.to_ne_bytes());
    message.extend_from_slice(&0u16.to_ne_bytes());
    message.extend_from_slice(&sequence.to_ne_bytes());
    message.extend_from_slice(&0u32.to_ne_bytes());
    message.extend_from_slice(&CN_IDX_PROC.to_ne_bytes());
    message.extend_from_slice(&CN_VAL_PROC.to_ne_bytes());
    message.extend_from_slice(&sequence.to_ne_bytes());
    message.extend_from_slice(&ack.to_ne_bytes());
    message.extend_from_slice(&4u16.to_ne_bytes());
    message.extend_from_slice(&0u16.to_ne_bytes());
    message.extend_from_slice(&operation.to_ne_bytes());
    message
}

/// Appends the messages of one datagram to `messages`. Threads are not
/// processes: a thread that starts is no change, and one that exits only
/// changes its process when it leads it.
pub fn parse(datagram: &[u8], messages: &mut Vec<Message>) {
    let mut rest = datagram;
    while rest.len() >= NLMSG_HDR_LEN {
        let length = word(rest, 0).unwrap_or(0) as usize;
        if length < NLMSG_HDR_LEN || length > rest.len() {
            return;
        }
        if let Some(message) = message(&rest[NLMSG_HDR_LEN..length]) {
            messages.push(message);
        }
        rest = &rest[length.next_multiple_of(4).min(rest.len())..];
    }
}

fn message(message: &[u8]) -> Option<Message> {
    if word(message, 0)? != CN_IDX_PROC || word(message, 4)? != CN_VAL_PROC {
        return None;
    }
    let sequence = word(message, 8)?;
    let ack = word(message, 12)?;
    let proc_event = message.get(CN_MSG_LEN..)?;
    let cpu = word(proc_event, 4)?;
    let data = |at: usize| word(proc_event, EVENT_DATA_AT + at);
    let event = match word(proc_event, 0)? {
        PROC_EVENT_NONE => Event::Acknowledged {
            ack,
            error: data(0)?,
        },
        PROC_EVENT_FORK => {
            let (child_pid, child_tgid) = (data(8)?, data(12)?);
            if child_pid == child_tgid {
                Event::Changed(child_tgid)
            } else {
                Event::Other
            }
        }
        PROC_EVENT_EXEC => Event::Changed(data(4)?),
        PROC_EVENT_EXIT => {
            let (pid, tgid) = (data(0)?, data(4)?);
            if pid == tgid {
                Event::Changed(tgid)
            } else {
                Event::Other
            }
        }
        _ => Event::Other,
    };
    Some(Message {
        cpu,
        sequence,
        event,
    })
}

/// The next number expected from each CPU.
#[derive(Default)]
pub struct Sequences {
    next: HashMap<u32, u32>,
}

impl Sequences {
    /// Records `message`; `false` when an event its CPU numbered before it
    /// never arrived. The first event heard from a CPU starts its count.
    pub fn continues(&mut self, message: &Message) -> bool {
        if matches!(message.event, Event::Acknowledged { .. }) {
            return true;
        }
        let expected = self
            .next
            .insert(message.cpu, message.sequence.wrapping_add(1));
        expected.is_none_or(|expected| expected == message.sequence)
    }

    pub fn clear(&mut self) {
        self.next.clear();
    }
}

fn word(bytes: &[u8], at: usize) -> Option<u32> {
    let b = bytes.get(at..at + 4)?;
    Some(u32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A datagram as the kernel sends it: one netlink message carrying one
    /// `proc_event` from `cpu`, numbered `sequence`, whose data words are
    /// `data`.
    fn numbered(what: u32, cpu: u32, sequence: u32, ack: u32, data: &[u32]) -> Vec<u8> {
        let mut event = Vec::new();
        event.extend_from_slice(&what.to_ne_bytes());
        event.extend_from_slice(&cpu.to_ne_bytes());
        event.extend_from_slice(&123_456_789u64.to_ne_bytes());
        for word in data {
            event.extend_from_slice(&word.to_ne_bytes());
        }
        let length = NLMSG_HDR_LEN + CN_MSG_LEN + event.len();
        let mut message = Vec::new();
        message.extend_from_slice(&(length as u32).to_ne_bytes());
        message.extend_from_slice(&NLMSG_DONE.to_ne_bytes());
        message.extend_from_slice(&0u16.to_ne_bytes());
        message.extend_from_slice(&[0; 8]);
        message.extend_from_slice(&CN_IDX_PROC.to_ne_bytes());
        message.extend_from_slice(&CN_VAL_PROC.to_ne_bytes());
        message.extend_from_slice(&sequence.to_ne_bytes());
        message.extend_from_slice(&ack.to_ne_bytes());
        message.extend_from_slice(&(event.len() as u16).to_ne_bytes());
        message.extend_from_slice(&0u16.to_ne_bytes());
        message.extend_from_slice(&event);
        message
    }

    fn datagram(what: u32, ack: u32, data: &[u32]) -> Vec<u8> {
        numbered(what, 3, 0, ack, data)
    }

    fn messages_of(datagram: &[u8]) -> Vec<Message> {
        let mut messages = Vec::new();
        parse(datagram, &mut messages);
        messages
    }

    fn events_of(datagram: &[u8]) -> Vec<Event> {
        messages_of(datagram)
            .into_iter()
            .map(|message| message.event)
            .collect()
    }

    #[test]
    fn a_new_process_an_exec_and_an_exit_each_change_their_process() {
        let fork = datagram(PROC_EVENT_FORK, 0, &[10, 10, 42, 42]);
        let exec = datagram(PROC_EVENT_EXEC, 0, &[43, 43]);
        let exit = datagram(PROC_EVENT_EXIT, 0, &[44, 44, 0, 17, 1, 1]);

        assert_eq!(events_of(&fork), [Event::Changed(42)]);
        assert_eq!(events_of(&exec), [Event::Changed(43)]);
        assert_eq!(events_of(&exit), [Event::Changed(44)]);
    }

    #[test]
    fn an_exec_from_a_thread_changes_its_process() {
        let exec = datagram(PROC_EVENT_EXEC, 0, &[51, 50]);

        assert_eq!(events_of(&exec), [Event::Changed(50)]);
    }

    #[test]
    fn a_thread_starting_or_ending_changes_no_process() {
        let thread_start = datagram(PROC_EVENT_FORK, 0, &[10, 10, 61, 60]);
        let thread_end = datagram(PROC_EVENT_EXIT, 0, &[61, 60, 0, 0, 1, 1]);

        assert_eq!(events_of(&thread_start), [Event::Other]);
        assert_eq!(events_of(&thread_end), [Event::Other]);
    }

    #[test]
    fn events_about_ids_and_names_are_not_process_changes() {
        let uid = datagram(0x4, 0, &[70, 70, 1000, 1000]);
        let comm = datagram(0x200, 0, &[70, 70, 0x6f6f_6f66, 0, 0, 0]);

        assert_eq!(events_of(&uid), [Event::Other]);
        assert_eq!(events_of(&comm), [Event::Other]);
    }

    #[test]
    fn reads_the_cpu_and_the_number_of_an_event() {
        let exec = numbered(PROC_EVENT_EXEC, 5, 77, 0, &[43, 43]);

        assert_eq!(
            messages_of(&exec),
            [Message {
                cpu: 5,
                sequence: 77,
                event: Event::Changed(43)
            }]
        );
    }

    #[test]
    fn reads_the_acknowledgement_of_a_listen_request_with_its_error() {
        let refused = datagram(PROC_EVENT_NONE, 8, &[1]);

        assert_eq!(
            events_of(&refused),
            [Event::Acknowledged { ack: 8, error: 1 }]
        );
    }

    #[test]
    fn reads_every_message_of_a_datagram_and_stops_at_a_truncated_one() {
        let mut both = datagram(PROC_EVENT_EXEC, 0, &[80, 80]);
        both.extend(datagram(PROC_EVENT_EXEC, 0, &[81, 81]));
        let truncated = &datagram(PROC_EVENT_EXEC, 0, &[82, 82])[..30];

        assert_eq!(events_of(&both), [Event::Changed(80), Event::Changed(81)]);
        assert_eq!(events_of(truncated), []);
    }

    #[test]
    fn messages_of_another_connector_are_ignored() {
        let mut other = datagram(PROC_EVENT_EXEC, 0, &[90, 90]);
        other[NLMSG_HDR_LEN..NLMSG_HDR_LEN + 4].copy_from_slice(&7u32.to_ne_bytes());

        assert_eq!(events_of(&other), []);
    }

    fn event(cpu: u32, sequence: u32) -> Message {
        Message {
            cpu,
            sequence,
            event: Event::Other,
        }
    }

    #[test]
    fn events_numbered_in_a_row_on_each_cpu_lose_nothing() {
        let mut sequences = Sequences::default();

        let heard = [event(0, 7), event(1, 90), event(0, 8), event(1, 91)]
            .iter()
            .all(|message| sequences.continues(message));

        assert!(heard);
    }

    #[test]
    fn a_gap_in_a_cpu_numbers_is_a_lost_event() {
        let mut sequences = Sequences::default();
        sequences.continues(&event(2, 7));

        assert!(!sequences.continues(&event(2, 9)));
        assert!(sequences.continues(&event(2, 10)));
    }

    #[test]
    fn a_cpu_count_wraps_around() {
        let mut sequences = Sequences::default();
        sequences.continues(&event(0, u32::MAX));

        assert!(sequences.continues(&event(0, 0)));
    }

    #[test]
    fn an_acknowledgement_is_not_counted() {
        let mut sequences = Sequences::default();
        sequences.continues(&event(0, 7));
        let ack = Message {
            cpu: u32::MAX,
            sequence: 1,
            event: Event::Acknowledged { ack: 2, error: 0 },
        };

        assert!(sequences.continues(&ack));
        assert!(sequences.continues(&event(0, 8)));
    }

    #[test]
    fn a_cleared_count_starts_again_from_the_next_event() {
        let mut sequences = Sequences::default();
        sequences.continues(&event(0, 7));

        sequences.clear();

        assert!(sequences.continues(&event(0, 50)));
    }

    #[test]
    fn an_ignore_request_names_the_ignore_operation() {
        let request = ignore_request(6);

        assert_eq!(word(&request, NLMSG_HDR_LEN), Some(CN_IDX_PROC));
        assert_eq!(
            word(&request, NLMSG_HDR_LEN + CN_MSG_LEN),
            Some(PROC_CN_MCAST_IGNORE)
        );
    }

    #[test]
    fn a_listen_request_names_the_proc_connector_and_the_listen_operation() {
        let request = listen_request(5, 9);

        assert_eq!(word(&request, 0), Some(request.len() as u32));
        assert_eq!(word(&request, NLMSG_HDR_LEN), Some(CN_IDX_PROC));
        assert_eq!(word(&request, NLMSG_HDR_LEN + 4), Some(CN_VAL_PROC));
        assert_eq!(word(&request, NLMSG_HDR_LEN + 12), Some(9));
        assert_eq!(
            word(&request, NLMSG_HDR_LEN + CN_MSG_LEN),
            Some(PROC_CN_MCAST_LISTEN)
        );
    }
}
