# Bundled YuNet face detector

Upstream: https://github.com/opencv/opencv_zoo/tree/main/models/face_detection_yunet

File: `face_detection_yunet_2023mar.onnx` (232589 bytes).
SHA-256: `8f2383e4dd3cfbb4553ea8718107fc0423210dc964f9f4280604804ed2552fa4`.
Verified against the upstream Git LFS pointer before inclusion. MIT licence in LICENSE.

The model supplies boxes and five 2D landmarks, not a 3D face mesh or expression recognition. Inference is local and the model is bundled; no image upload or first-use download is needed. Source is resized with preserved aspect and padded to the fixed 640 × 640 model input, with coordinates converted back to source fractions.
