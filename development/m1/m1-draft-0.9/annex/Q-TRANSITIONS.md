# Annex Q3 (part): replyState × ownState (D102, I54)

Source: browser.md:447–473. The machine-readable form is `../vectors/storequeue-transitions.json`. The runner checks every recorded change against it, and checks the pair of every message after every event.

## States

- `replyState ∈ {none, sent, suppressed}`. The final reply is sent at most once and never changes ownership.
- `ownState ∈ {queued, ready, active, released}`.
  - `queued`: not yet evaluated.
  - `ready`: passed the quota check and is waiting for a seat.

## Diagram

```
                 admit (B4)
                     |
                     v
            (none, queued) --evalHead ok--> (none, ready) --seat + set--> (none, active)
             |    |    |                       |      |                     |     |     |
     quota   |    |    | cancel     expiry     |      | cancel     deadline |     |     | teardown
     4300 /  |    |    | (suppressed)  store   |      | (suppressed)  store-|     |     | (no reply ever)
     expiry  |    |    |                       |      |            Timeout  |     |     v
     store   v    |    v                       v      v                     v     |  (suppressed, active)
     (sent, released)  (suppressed, released)  (sent, released) (suppressed,  (sent, active)   |
                                                                   released)      |  settle   | settle
                                                                                  v           v
                                        settle before deadline:       (sent, released)  (suppressed, released)
                                        (none, active) -> reply null/transport -> (sent, released)
```

## Table: the only allowed composite transitions

| # | From (reply, own) | Event | Reply emitted | To (reply, own) | Release | Seat/lock |
|---|---|---|---|---|---|---|
| T1 | — | admit, counters fit | none | (none, queued) | — | — |
| T2 | (none, queued) | evalHead, quota ok | none | (none, ready) | no | none held |
| T3 | (none, queued) | evalHead, quota exceeded | 4300 `{quota, 1048576}` | (sent, released) | yes, same step | none ever |
| T4 | (none, queued) | `STORE_QUEUE_WAIT` at accept+10000 | −32005 `{store}` | (sent, released) | yes | none |
| T5 | (none, queued) | cancelFrame / session end | — | (suppressed, released) | yes | none |
| T6 | (none, ready) | seat free, oldest by acceptance | none | (none, active) | no | takes seat + lock |
| T7 | (none, ready) | `STORE_QUEUE_WAIT` | −32005 `{store}` | (sent, released) | yes | none |
| T8 | (none, ready) | cancelFrame / session end | — | (suppressed, released) | yes | none |
| T9 | (none, active) | settle ok before deadline | `null` | (sent, released) | yes | freed |
| T10 | (none, active) | settle fail before deadline | −32603 `{transport}` | (sent, released) | yes, after reload | freed |
| T11 | (none, active) | `STORE_WRITE_DEADLINE` at start+5000 | −32603 `{storeTimeout}` | (sent, active) | **no** | **kept** |
| T12 | (sent, active) | settle ok or fail | none (reload) | (sent, released) | yes | freed |
| T13 | (none, active) | frame torn down / session end | none | (suppressed, active) | **no** | **kept** |
| T14 | (suppressed, active) | settle ok or fail | none (reload) | (suppressed, released) | yes | freed |
| T15 | (sent, active) | frame torn down | none | (sent, active) | no | kept |
| T16 | (suppressed, active) | deadline | none (P-Q1-5) | (suppressed, active) | no | kept |

Stable pairs between events: (none, queued), (none, ready), (none, active), (sent, active), (suppressed, active), (sent, released), (suppressed, released).

**Forbidden:**
- (none, released) at any time;
- (sent, queued) or (sent, ready) between events, which is what `quotaNoRelease` leaves;
- a second reply from (sent, ·);
- `active → released` without a settle, which is what `storeEarlyRelease` does.

**Invariant G** (browser.md:471; validation.md:292–296):
- In every line, the session and global counters equal the number of unreleased messages and the sum of their `qbytes`.
- No counter is negative.
- `globQ ≥ sessQ` for every session.
