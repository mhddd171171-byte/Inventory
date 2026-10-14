# Basic user guide

Inventory Manager opens directly to the Dashboard. Amounts use OMR with three decimal places.

## Products and Stock Entry

Each product needs a unique positive numeric **Item Code**, a name, and model/description. Search Products by code, name, or model. Use **Add product** for a product record with no incoming quantity, or **Edit** to correct its information.

To receive stock, open **Stock Entry**. Enter the Item Code; an existing code automatically fills the product name/model. For a new code, enter those fields. Enter a positive whole-number quantity and purchase cost per unit, then save. Zero purchase cost is permitted. Each delivery keeps its original cost and date; new costs never overwrite old batches.

## Stock Out and CR

Open **Stock Out**. Enter the customer's external numeric **CR number** and one or more lines. Each line requires an existing Item Code, positive quantity no greater than available stock, and the actual nonnegative selling price per unit. Product details and availability fill automatically. CR is separate from Item Code; this app never generates a CR.

Use **Add product line** for multiple parts under the same CR. Save validates every line and commits all quantities together. An invalid line saves nothing and preserves your input. The same CR can be reused in a later transaction. Purchase cost is calculated FIFO; revenue minus that cost is gross profit. Search the recorded sales by CR to view their lines.

## Dashboard and product history

The Dashboard shows product types, available units, current purchase value, monthly revenue, monthly gross profit, and low-stock products. Quantity 2 is **Low Stock**, 1 is **Critical Stock**, and 0 is **Out of Stock**. Product details show incoming batches and Stock Out history.

Old migrated records with unavailable prices or costs show **Unknown**. Known subtotal values are identified separately; missing data is never silently counted as zero.

## Reports

Choose **Reports**, then Monthly Sales, Current Inventory, or Monthly Inventory Summary. Select a month for monthly reports. Choose **Export PDF** or **Export Excel (.xlsx)** and a local save location. Reports are generated offline from fresh database data. The remaining quantity is the current balance even when selecting a past sales month.

## AI Assistant

The model starts automatically with the app. Wait for **AI Ready**, then ask in English or Arabic. Examples: "How many screens do we have?", "How many did we sell this month?", "Find a screen for Samsung S3", "I don't have the Item Code", "What was sold under CR 99876?", or "How do I add a product?".

The conversation remembers selected products, candidate lists and prior searches. Choose a listed result when a question is ambiguous. **New conversation** clears that chat's context. Product searches do not require an Item Code. Data answers come from the real database; requests to change stock receive manual instructions instead of making changes.

Choose the language and **Start Conversation** to talk continuously. After you finish speaking, the assistant transcribes locally, answers, speaks through your speakers, and listens again. **End Conversation** stops listening and playback. Listening pauses while the assistant speaks. Navigation away from the assistant ends Voice Mode. Speech recognition can make mistakes; verify important quantities in the inventory screen.

## Backup

Choose **Backup & restore** in the sidebar. Save backups on local or removable storage. Restore replaces the current inventory with a validated backup and saves a recovery copy first. Back up regularly and before moving computers.
