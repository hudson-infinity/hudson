# yoagent

Rust loop library; MIT. Reviewed 2026-09-20.

Repository: [yologdev/yoagent](https://github.com/yologdev/yoagent). Source snapshot: `c2ed483a31be96052061b2d2557f9c75929de4fb`.

## What we inspected

The loop separates model/tool continuation from queued follow-ups and checks cancellation between stages. Tool execution passes through middleware that can allow, modify, or deny calls. The inspected dispatch path converts middleware failure into a denial.

## Conceptual shape

Original reading aid, not copied upstream code or an exact reconstruction:

```text
consume steering
→ stream model reply
→ pass calls through tool middleware
→ execute and append results
→ repeat for tools or pending messages
→ settle when no more work remains
```

## What Hudson can learn

Keep streaming events and control messages explicit. Use a common tool interception point across sequential and parallel paths.

## Limits of this reference

A middleware callback is not a sandbox boundary. If arguments change, Hudson must authorize the final request and bind any approval to it. Crash-safe approval waits and external operation reconciliation still need proof.

## Source pointers

- [LICENSE](https://github.com/yologdev/yoagent/blob/c2ed483a31be96052061b2d2557f9c75929de4fb/LICENSE)
- [src/agent_loop.rs](https://github.com/yologdev/yoagent/blob/c2ed483a31be96052061b2d2557f9c75929de4fb/src/agent_loop.rs)
- [src/types.rs](https://github.com/yologdev/yoagent/blob/c2ed483a31be96052061b2d2557f9c75929de4fb/src/types.rs)
- [src/agent.rs](https://github.com/yologdev/yoagent/blob/c2ed483a31be96052061b2d2557f9c75929de4fb/src/agent.rs)

## Related offline examples

- [tool_cycle.rs](../examples/tool_cycle.rs)
- [parallel_tools.rs](../examples/parallel_tools.rs)
- [steering_cancel.rs](../examples/steering_cancel.rs)

[Back to the collection](../README.md).
