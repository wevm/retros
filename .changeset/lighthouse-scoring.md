---
'uphill': minor
---

Scored like Lighthouse, exactly. A metric's `poor` key sets the value that scores 50, Lighthouse's median control point, which defaults to twice the target. A check can be `skip` when it does not apply, which gives it no score, as Lighthouse gives a not applicable audit no weight. Curve scores now follow `Util.computeLogNormalScore`, which rounds down after a small boost above 90, and category means add scores as Lighthouse does, so some scores read one point lower. `examples/lighthouse` reproduces Lighthouse 13.6.0's category scores for each page.
