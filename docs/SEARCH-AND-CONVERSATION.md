# Search and conversation — 2.0.2

This update preserves the existing inventory application and the Arabic/English voices approved by the customer. It adds the requested Search workspace and improves conversation without new cloud services or inventory write tools.

## Search and exports

- **CR number**: exact numeric match across every saved transaction under that CR.
- **Item Code**: exact numeric match; combine with CR to identify that part sold under that CR.
- **Search text**: partial product name, device model, Arabic description, code, CR or saved legacy sale reference. Multiple words must all match.
- **Dates**: inclusive local calendar bounds for transactions. Current quantities are labeled separately and do not represent historical closing stock.
- **Suggestions**: actual saved CRs, item codes, names and models. Empty inventory produces no invented suggestions.

Sale lines show their actual quantity, selling price, FIFO cost, revenue and profit. CR does not apply to incoming batches. PDF and Excel exports refresh the approved database snapshot and include all matching rows; multi-section Excel exports use separate filtered worksheets. Missing migrated costs/prices remain Unknown.

## Conversation

The same evaluated Qwen model recognizes greetings, thanks, farewells and conversational followups through its semantic plan. Social turns retain selected products. Inventory facts still come from validated read-only backend results. Answers are brief by default, with the full result available under **More details**. Voice speaks the brief answer.

During reply playback, bundled WebRTC echo cancellation processes microphone audio locally. Sustained user speech stops queued output and the same capture stream retains the start of the user's question. The frontend immediately shows Listening, marks the interrupted reply and uses that captured transcript for the next turn. Saying goodbye ends Voice Mode. End Conversation always cancels the owned worker.

No audio is uploaded or stored by the application. Actual device echo, room acoustics and recognition quality still affect interruption behavior; the automated simulated duplex checks and physical-device playback checks are described in VALIDATION.md.
