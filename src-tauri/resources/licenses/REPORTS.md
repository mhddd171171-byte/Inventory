# Offline PDF and Excel reports

The existing report renderer now supports exactly three reports in both PDF and `.xlsx`:

| Report | Content |
|---|---|
| Monthly Sales | Month, Item Code, product, sold quantity, external CR, actual selling unit price, FIFO cost, revenue, profit; totals for units/cost/revenue/profit |
| Current Inventory | Item Code, product, available quantity, purchase value, stock status |
| Monthly Inventory Summary | Product code/name, selected-month sold quantity, current remaining quantity, FIFO cost, revenue and profit |

The frontend requests a fresh SQLite snapshot for the selected month before export. The backend accepts only valid report bytes and uses a native local save dialog. No upload or report service is involved.

PDFs use locally embedded Noto Sans / Noto Sans Arabic fonts, jsPDF and AutoTable. Tables repeat headers across pages; reports have page numbering. Arabic names are shaped for display. ExcelJS produces actual OOXML workbooks with headers, filters, frozen rows, and OMR formats. Item Codes and CRs remain text, including long numeric references; customer text is never assigned as an Excel formula.

Prices originate as integer baisa and use three decimal places. Unknown migrated costs/prices/profits remain **Unknown** rather than zero. The monthly inventory summary labels remaining quantity as the current balance, not a reconstructed historical closing balance.

Automated checks cover each PDF and workbook, month consistency, empty inventory, Arabic rows, formula-like names, long codes, unknown legacy values and multi-page output. Rendered PDF previews were inspected locally for layout and readable Arabic. See [validation](VALIDATION.md) for current release results.
