#!/usr/bin/env python3
"""Generate golden snapshot JSON fixtures for all Foundation-layer types.

Usage: python3 generate_fixtures.py [output_dir]
Default output_dir: tests/compatibility/fixtures/

This script generates representative JSON samples for:
- Msg (all 3 roles)
- All 6 ContentBlock types
- All 28 EventType variants (sample key events)
- AgentState
- Task

These serve as golden snapshots for Rust vs Python diff testing.
"""

import json
import sys
from pathlib import Path
from datetime import datetime, timezone


def fixture(name: str) -> dict:
    """Create a minimal fixture metadata wrapper."""
    return {
        "_fixture_name": name,
        "_generated_at": datetime.now(timezone.utc).isoformat(),
        "_description": f"Golden snapshot fixture for {name}",
        "data": None,
    }


def generate_fixtures(output_dir: Path) -> list[str]:
    """Generate all fixtures. Returns list of filenames created."""
    output_dir.mkdir(parents=True, exist_ok=True)
    files = []

    # ── Msg fixtures ──────────────────────────────────────────────
    # User message
    f = fixture("Msg-user-text")
    f["data"] = {
        "name": "user1",
        "content": [
            {
                "type": "text",
                "text": "Hello, what is the weather?",
                # id & created_at are dynamic — will be diff-normalized
            }
        ],
        "role": "user",
        "metadata": {},
        # finished_at = created_at for user messages
    }
    path = output_dir / "msg_user_text.json"
    json.dump(f, path.open("w"), indent=2, ensure_ascii=False)
    files.append(path.name)

    # Assistant message
    f = fixture("Msg-assistant-text")
    f["data"] = {
        "name": "assistant",
        "content": [{"type": "text", "text": "The weather is sunny today."}],
        "role": "assistant",
        "metadata": {},
    }
    path = output_dir / "msg_assistant_text.json"
    json.dump(f, path.open("w"), indent=2, ensure_ascii=False)
    files.append(path.name)

    # System message
    f = fixture("Msg-system-text")
    f["data"] = {
        "name": "system",
        "content": [{"type": "text", "text": "You are a helpful assistant."}],
        "role": "system",
        "metadata": {},
    }
    path = output_dir / "msg_system_text.json"
    json.dump(f, path.open("w"), indent=2, ensure_ascii=False)
    files.append(path.name)

    # Assistant with tool call
    f = fixture("Msg-assistant-with-tool-call")
    f["data"] = {
        "name": "assistant",
        "content": [
            {"type": "text", "text": "Let me search for that."},
            {
                "type": "tool_call",
                "id": "call-001",
                "name": "search",
                "input": '{"query": "weather in Beijing"}',
                "state": "submitted",
            },
        ],
        "role": "assistant",
        "metadata": {},
    }
    path = output_dir / "msg_assistant_with_tool_call.json"
    json.dump(f, path.open("w"), indent=2, ensure_ascii=False)
    files.append(path.name)

    # ── ContentBlock fixtures ─────────────────────────────────────
    f = fixture("ContentBlock-Text")
    f["data"] = {"type": "text", "text": "Hello, world!"}
    path = output_dir / "content_block_text.json"
    json.dump(f, path.open("w"), indent=2, ensure_ascii=False)
    files.append(path.name)

    f = fixture("ContentBlock-Thinking")
    f["data"] = {"type": "thinking", "thinking": "Let me reason about this..."}
    path = output_dir / "content_block_thinking.json"
    json.dump(f, path.open("w"), indent=2, ensure_ascii=False)
    files.append(path.name)

    f = fixture("ContentBlock-Hint")
    f["data"] = {"type": "hint", "hint": "Please respond in JSON format", "source": "system"}
    path = output_dir / "content_block_hint.json"
    json.dump(f, path.open("w"), indent=2, ensure_ascii=False)
    files.append(path.name)

    f = fixture("ContentBlock-Data-base64")
    f["data"] = {
        "type": "data",
        "source": {
            "type": "base64",
            "data": "SGVsbG8gV29ybGQ=",
            "media_type": "text/plain",
        },
    }
    path = output_dir / "content_block_data_base64.json"
    json.dump(f, path.open("w"), indent=2, ensure_ascii=False)
    files.append(path.name)

    f = fixture("ContentBlock-Data-url")
    f["data"] = {
        "type": "data",
        "source": {
            "type": "url",
            "url": "https://example.com/image.png",
            "media_type": "image/png",
        },
    }
    path = output_dir / "content_block_data_url.json"
    json.dump(f, path.open("w"), indent=2, ensure_ascii=False)
    files.append(path.name)

    f = fixture("ContentBlock-ToolCall")
    f["data"] = {
        "type": "tool_call",
        "id": "call-abc",
        "name": "get_weather",
        "input": '{"city": "Beijing"}',
        "state": "pending",
    }
    path = output_dir / "content_block_tool_call.json"
    json.dump(f, path.open("w"), indent=2, ensure_ascii=False)
    files.append(path.name)

    f = fixture("ContentBlock-ToolResult")
    f["data"] = {
        "type": "tool_result",
        "id": "result-001",
        "name": "search",
        "output": "Found 3 results",
        "state": "success",
    }
    path = output_dir / "content_block_tool_result.json"
    json.dump(f, path.open("w"), indent=2, ensure_ascii=False)
    files.append(path.name)

    # ── Event fixtures ────────────────────────────────────────────
    f = fixture("Event-ReplyStart")
    f["data"] = {
        "type": "REPLY_START",
        "session_id": "session-001",
        "reply_id": "reply-001",
        "name": "agent",
        "role": "assistant",
    }
    path = output_dir / "event_reply_start.json"
    json.dump(f, path.open("w"), indent=2, ensure_ascii=False)
    files.append(path.name)

    f = fixture("Event-ReplyEnd")
    f["data"] = {
        "type": "REPLY_END",
        "session_id": "session-001",
        "reply_id": "reply-001",
        "finished_reason": "completed",
    }
    path = output_dir / "event_reply_end.json"
    json.dump(f, path.open("w"), indent=2, ensure_ascii=False)
    files.append(path.name)

    f = fixture("Event-TextBlockDelta")
    f["data"] = {
        "type": "TEXT_BLOCK_DELTA",
        "reply_id": "reply-001",
        "block_id": "block-001",
        "delta": "Hello",
    }
    path = output_dir / "event_text_block_delta.json"
    json.dump(f, path.open("w"), indent=2, ensure_ascii=False)
    files.append(path.name)

    f = fixture("Event-ToolCallStart")
    f["data"] = {
        "type": "TOOL_CALL_START",
        "reply_id": "reply-001",
        "tool_call_id": "tc-001",
        "tool_call_name": "search",
    }
    path = output_dir / "event_tool_call_start.json"
    json.dump(f, path.open("w"), indent=2, ensure_ascii=False)
    files.append(path.name)

    f = fixture("Event-UserInterrupt")
    f["data"] = {
        "type": "USER_INTERRUPT",
        "reply_id": "reply-001",
    }
    path = output_dir / "event_user_interrupt.json"
    json.dump(f, path.open("w"), indent=2, ensure_ascii=False)
    files.append(path.name)

    # ── AgentState fixture ────────────────────────────────────────
    f = fixture("AgentState-default")
    f["data"] = {
        "session_id": "test-session-001",
        "summary": "",
        "context": [],
        "reply_context": {"reply_id": "reply-001", "cur_iter": 0},
        "permission_context": {},
        "tool_context": {
            "max_cache_files": 100,
            "max_cache_bytes": 25000.0,
            "read_file_cache": [],
            "activated_groups": [],
        },
        "tasks_context": {"tasks": []},
        "middle_context": {},
    }
    path = output_dir / "agent_state_default.json"
    json.dump(f, path.open("w"), indent=2, ensure_ascii=False)
    files.append(path.name)

    # ── Planner + ReActAgent compatibility fixtures ─────────────────────
    planner_fixtures = {
        "planner_success_trace.json": {
            "name": "Planner-successful-planned-task",
            "description": "Planner creates a plan, executes one step, and completes.",
            "events": [
                "planning_started",
                "planning_completed",
                "step_started",
                "step_completed",
                "task_completed",
            ],
            "steps": [{"status": "completed", "objective": "answer directly"}],
            "outcome": {"completed": {"summary": "planned task completed"}},
        },
        "planner_tool_step_trace.json": {
            "name": "Planner-tool-step",
            "description": "Planner step delegates deterministic tool work through ReAct behavior.",
            "events": [
                "planning_started",
                "planning_completed",
                "step_started",
                "tool_call_start",
                "tool_result_end",
                "step_completed",
                "task_completed",
            ],
            "tool_activity": [{"tool_name": "calculator", "arguments": {"expression": "2+2"}, "result_summary": "4"}],
            "outcome": {"completed": {"summary": "planned task completed"}},
        },
        "planner_replanning_trace.json": {
            "name": "Planner-replanning-after-recoverable-failure",
            "description": "Planner records a recoverable failure, replans, and completes replacement work.",
            "events": [
                "planning_started",
                "planning_completed",
                "step_started",
                "step_failed",
                "replanning_started",
                "replanning_completed",
                "step_started",
                "step_completed",
                "task_completed",
            ],
            "revision": {"trigger": "recoverable_failure", "rationale_contains": "recoverable"},
            "outcome": {"completed": {"summary": "planned task completed"}},
        },
        "planner_cancellation_trace.json": {
            "name": "Planner-cancellation",
            "description": "Planner cancellation is explicit and terminal.",
            "events": ["planning_started", "planning_completed", "step_cancelled", "task_cancelled"],
            "outcome": {"cancelled": {"reason": "cancelled"}},
        },
        "planner_unsupported_trace.json": {
            "name": "Planner-unsupported-capability",
            "description": "Unsupported/deferred Planner capability is surfaced instead of silently simulated.",
            "events": ["task_unsupported"],
            "outcome": {
                "unsupported": {
                    "reason": "capability is outside Feature 021 scope",
                    "capability": "parallel DAG scheduling",
                }
            },
        },
    }

    for filename, spec in planner_fixtures.items():
        f = fixture(spec["name"])
        f["_description"] = spec["description"]
        f["data"] = {
            "trace_id": "trace-001",
            "task_id": "task-001",
            "normalized_fields": ["trace_id", "task_id", "plan_id", "step_id", "timestamp"],
            "events": [
                {"sequence": index + 1, "event_type": event, "task_id": "task-001"}
                for index, event in enumerate(spec["events"])
            ],
            "steps": spec.get("steps", []),
            "tool_activity": spec.get("tool_activity", []),
            "revision": spec.get("revision"),
            "final_outcome": spec["outcome"],
        }
        path = output_dir / filename
        json.dump(f, path.open("w"), indent=2, ensure_ascii=False)
        files.append(path.name)

    # ── Task fixture ──────────────────────────────────────────────
    f = fixture("Task-pending")
    f["data"] = {
        "subject": "Implement login",
        "description": "Add OAuth2 authentication flow",
        "metadata": {},
        "state": "pending",
        "blocks": [],
        "blocked_by": [],
    }
    path = output_dir / "task_pending.json"
    json.dump(f, path.open("w"), indent=2, ensure_ascii=False)
    files.append(path.name)

    f = fixture("Task-in-progress-with-deps")
    f["data"] = {
        "subject": "Deploy to production",
        "description": "Release v2.0 to production environment",
        "metadata": {"priority": "high"},
        "state": "in_progress",
        "owner": "alice",
        "blocks": ["task-deploy-docs"],
        "blocked_by": ["task-ci-pass"],
    }
    path = output_dir / "task_in_progress.json"
    json.dump(f, path.open("w"), indent=2, ensure_ascii=False)
    files.append(path.name)

    return files


def main():
    output_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).parent / "fixtures"
    files = generate_fixtures(output_dir)
    print(f"Generated {len(files)} golden snapshot fixtures in {output_dir}")
    for name in sorted(files):
        print(f"  - {name}")
    print("\nDone. These fixtures serve as the Python reference for Rust diff tests.")


if __name__ == "__main__":
    main()
