# Multi-Dimensional Context Pipeline

We capture context through a three-stage progressive pipeline (Host Process Metadata -> UIA Paragraph Expansion -> Local Visual Snip Fallback) and bundle it into a standardized Context Envelope. We do this because raw system-wide text selections lack document hierarchy outside browser DOMs, and grounding explanations in the surrounding paragraph is essential for accurate technical disambiguation across heterogeneous desktop environments.
