# Basic user guide

Inventory Manager opens directly to the Dashboard. Amounts use OMR with three decimal places.

## Products and Stock Entry

Each product needs a unique positive numeric **Item Code**, a name, and model/description. Search Products by code, name, or model. Use **Add product** for a product record with no incoming quantity, or **Edit** to correct its information.

To receive stock, open **Stock Entry**. Enter the Item Code; an existing code automatically fills the product name/model. For a new code, enter those fields. Enter a positive whole-number quantity and purchase cost per unit, then save. Zero purchase cost is permitted. Each delivery keeps its original cost and date; new costs never overwrite old batches.

**Buying Price per Unit** is the amount you paid for one piece. **Total Bought For** is the received quantity multiplied by that buying price. Each delivery can have a different price. **Selling Price per Unit** is what the customer paid for one piece in Stock Out; **Total Sold For** is quantity sold multiplied by that price.

**Total Buying Cost** in a sale is what you originally paid for those sold pieces. FIFO means “first in, first out”: the app uses the oldest purchase batches first. Example: buy five pieces at OMR 10.000, then three at OMR 12.000. Selling six pieces uses buying cost OMR 62.000 (5 × 10 + 1 × 12). If you sell each for OMR 15.000, the total sold amount is OMR 90.000 and gross profit is OMR 28.000. This is an explanation, not an actual inventory record. Batch history shows the original buying prices; do not assume a single buying price when batches differ.

## Stock Out and CR

Open **Stock Out**. Enter the customer's external numeric **CR number** and one or more lines. Each line requires an existing Item Code, positive quantity no greater than available stock, and the actual nonnegative selling price per unit. Product details and availability fill automatically. CR is separate from Item Code; this app never generates a CR.

Use **Add product line** for multiple parts under the same CR. Save validates every line and commits all quantities together. An invalid line saves nothing and preserves your input. The same CR can be reused in a later transaction. Purchase cost is calculated FIFO; revenue minus that cost is gross profit. Search the recorded sales by CR to view their lines.

## Dashboard and product history

The Dashboard shows product types, available units, current purchase value, monthly revenue, monthly gross profit, and low-stock products. Quantity 2 is **Low Stock**, 1 is **Critical Stock**, and 0 is **Out of Stock**. Product details show incoming batches and Stock Out history.

Old migrated records with unavailable prices or costs show **Unknown**. Known subtotal values are identified separately; missing data is never silently counted as zero.

## Reports

Choose **Reports**, then Monthly Sales, Current Inventory, or Monthly Inventory Summary. Select a month for monthly reports. Choose **Export PDF** or **Export Excel (.xlsx)** and a local save location. Reports are generated offline from fresh database data. The remaining quantity is the current balance even when selecting a past sales month.

## Search workspace

Open **Search** in the sidebar. Enter an exact **CR number** to see its sold items, or combine CR with an exact **Item Code** to find that part under that CR. Repeated transactions under the same CR are included. Search text also accepts partial product names, device models, Arabic descriptions and saved references. Suggestions come from actual saved records. Optional **From date** and **To date** include both endpoints using your computer's local dates.

All filters apply together. The page shows matching sale lines, current product balances and incoming stock entries. CR applies to sales, so incoming deliveries are hidden when CR is set. Use **Export search PDF** or **Export search Excel** to save every matching row, including rows beyond the screen's 100-row display limit. Excel separates sales, current products and incoming batches into individual sheets. Clear filters to browse all records.

## AI Assistant

The model starts automatically with the app. Wait for **AI Ready**, then ask in English or Arabic. Examples: "How many screens do we have?", "How many did we sell this month?", "Find a screen for Samsung S3", "I don't have the Item Code", "What was sold under CR 99876?", or "How do I add a product?".

The conversation remembers selected products, candidate lists and prior searches. Choose a listed result when a question is ambiguous. **New conversation** clears that chat's context. Product searches do not require an Item Code. Data answers come from the real database; requests to change stock receive manual instructions instead of making changes.

Voice and text use the same approved read-only lookup functions. Ask “Show CRR ID 99876”, “Show item ID 11234”, or “What buying and selling prices are recorded for item 11234?”. CR summaries include actual buying cost and sold amount. A CR from the external repair system is available only after its Stock Out is recorded here. The assistant has no unrestricted SQL access and cannot look up data absent from this application's database.

Greetings and everyday conversational replies are welcome. Answers are brief by default; expand **More details** to inspect the complete result. A greeting does not forget the product you were discussing.

Choose the language and **Start Conversation** to talk continuously. Pause for about two seconds when your question is finished. You can hesitate briefly or ask a longer question; the assistant waits, transcribes locally, answers briefly and speaks. To interrupt a reply, start speaking: the assistant stops playback and captures your question without reopening the microphone. The interrupted reply is marked in the chat. Say goodbye to finish, or use **End Conversation** to stop immediately. Navigation away ends Voice Mode. Speech recognition can make mistakes; verify important quantities in the inventory screen. Arabic speech uses Modern Standard Arabic. See [VOICE-IMPROVEMENTS.md](VOICE-IMPROVEMENTS.md) for model evaluation.

## Backup

Choose **Backup & restore** in the sidebar. Save backups on local or removable storage. Restore replaces the current inventory with a validated backup and saves a recovery copy first. Back up regularly and before moving computers.
