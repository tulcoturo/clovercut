# Caption and effect qualification

Full CI run [41895228648](https://github.com/tulcoturo/clovercut/actions/runs/41895228648)
passed on `da55ea890e442e0174ed1ff20662c8418f1234b9`: strict workspace Clippy,
1341 ordinary tests, documentation examples, strict gRPC Clippy, 30 gRPC tests,
14 selected virtual-desktop journeys and audio connector regressions.
This covers Remove Black, saved caption-language selection, speech-reel input
and original-timeline mapping, including the corrected 480×270 effect card.
The desktop tests ran on virtual display/audio; no physical Mac or downloaded
Whisper recognition-quality validation is implied.

PR #20 was merged into its prerequisite PR #19. The resulting source tree
`19d9c5e0571f93b9f45827f09e5ada91ffe6e184` is identical to the qualified source
tree above (`git diff --quiet` succeeds). PR #19's separate run 41895198601
terminated during test compilation without completing qualification and its
job log was unavailable; it is not counted as a passing run. The combined
candidate uses the successful complete PR #20 run as its qualification.

Raw logs are retained locally under
`/home/turotulco/Work/clovercut-pr20-qualification`.
