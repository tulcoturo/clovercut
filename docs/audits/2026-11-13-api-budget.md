# API contract and GPU budget override

Two previously deferred features from the full upstream history are adapted.

- `80ed9457`: `clovercut --schema` prints the complete current API contract,
  with method replies, events, errors and shared type definitions. Clovercut's
  caption collections and typed tracks are included. The document uses a stable
  Clovercut URN rather than an upstream website URL. This mode exits before
  window/GPU initialization or file logging. Existing request schemas remain
  available to MCP clients.
- `fc60f4eb`: the window and compositor read the same cached memory-budget
  override. `CLOVERCUT_GPU_BUDGET` takes precedence over the upstream-compatible
  `CONCAT_GPU_BUDGET`. `off`, `none` and `0` disable the gates; 1–99 optionally
  followed by one percent sign changes the creation threshold and sets the loss
  threshold four points higher, capped at 99. Unset/invalid input preserves the
  existing thresholds. Repeated percent signs and fractional/out-of-range inputs
  are refused. The chosen override is logged once.

Local API tests passed 35/35, including contract reply/reference coverage. MCP
tests passed 12/12, including schema samples and the editable-shape size budget.
The GPU override parser regression passed. Native window compilation of the
launch flag and window-device configuration remains part of final CI; no full
GUI compilation or launch was performed locally.
