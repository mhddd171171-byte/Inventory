# FINAL DEVELOPMENT INSTRUCTIONS
# Al Mohtarif Repair Center — Offline Inventory Management System

You are continuing development of my existing Inventory Management System built with Tauri 2, React, TypeScript, Rust, and SQLite.

IMPORTANT:
This is a correction and refinement of the existing project based on the customer's actual requirements.

Do not rebuild the application from scratch.
Inspect the entire existing repository, preserve working components, and modify the current implementation.

The requirements in this message override all previous specifications.

Your objective is to deliver a simple, fast, reliable, fully offline inventory management application with an intelligent local AI assistant and full two-way voice conversation.

Do not add functionality outside the requirements below.

==================================================
1. DEVELOPMENT STRATEGY AND TARGET PLATFORMS
==================================================

We will develop and test the software on Windows first.

Development sequence:

Phase 1:
Complete all application functionality on Windows.

Phase 2:
Complete and test the local AI chatbot and two-way voice assistant on Windows.

Phase 3:
Build and test the Windows installer.

Phase 4:
Prepare the application for macOS on Intel-based MacBooks.

Phase 5:
Test macOS compatibility in a suitable environment, followed by final testing on the customer's physical MacBook.

The customer's computer is an Intel Core i5 MacBook.

The exact processor generation, macOS version, and RAM capacity are not yet confirmed.

Therefore:

- Maintain Windows and macOS compatibility.
- Use one shared source codebase.
- Do not use Windows-only functionality for core business logic.
- Use platform-specific runtime binaries when necessary.
- Do not sacrifice Windows compatibility.
- Do not assume the customer's MacBook has powerful AI hardware.
- Do not claim a macOS virtual machine test is equivalent to testing on physical Mac hardware.
- Do not claim macOS compatibility is verified until it has actually been tested.

Prioritize completing Windows development before macOS packaging.

==================================================
2. TECHNICAL ARCHITECTURE
==================================================

Keep the existing technology stack:

Desktop:
Tauri 2

Frontend:
React + TypeScript

Backend:
Rust

Database:
SQLite

AI Runtime:
llama.cpp

AI Model:
A suitable locally running, commercially redistributable open-source LLM.

Speech Recognition:
Offline open-source STT.

Speech Synthesis:
Offline open-source TTS.

Reports:
PDF and Excel (.xlsx).

The application must:

- Run entirely offline.
- Store all data locally.
- Operate on one computer.
- Require no cloud database.
- Require no online AI services.
- Require no API subscriptions.
- Require no external authentication.
- Require no internet connection during normal operation.

Only use free and open-source components with suitable licenses.

All AI models and voice components must operate locally.

==================================================
3. SINGLE-USER APPLICATION
==================================================

The application is intended for one user only.

There is no need for authentication.

REMOVE:

- Login screen.
- Username and password.
- Registration.
- Multiple accounts.
- User management.
- Administrator roles.
- Employee roles.
- Permission management.
- Authentication-related settings.

The application must open directly to the dashboard.

Do not request credentials during startup.

Remove unnecessary authentication dependencies and backend logic carefully.

Preserve existing inventory data during database migrations.

==================================================
4. PRODUCT CREATION AND ITEM CODES
==================================================

Every product must have a unique numeric Item Code.

Example:

Item Code: 11234
Product: iPhone 17 Pro Max LCD

The Item Code identifies the product type.

It is completely separate from the customer's CR number.

Required product information:

- Unique numeric Item Code.
- Product Name.
- Product Model / Description.

The user must be able to:

- Create products manually.
- Edit product information.
- Search products by Item Code.
- Search products by name or model.
- View available quantities.

When an existing Item Code is entered, automatically display the corresponding product.

Prevent duplicate Item Codes.

Do not implement barcode scanning or QR code scanning.

All operations must use manual input.

==================================================
5. STOCK ENTRY
==================================================

Implement a simple incoming inventory workflow.

Required inputs:

- Item Code.
- Product Name / Model for new products.
- Incoming Quantity.
- Purchase Cost per Unit.

Example:

Item Code: 11234
Product: iPhone 17 Pro Max LCD
Quantity: 5
Purchase Cost: OMR 10.000 per unit

When saved:

- Add 5 units to inventory.
- Record the purchase cost.
- Record the transaction date automatically.

If the Item Code already exists:

- Automatically display its product information.
- Add the new quantity to the existing stock.
- Do not create a duplicate product.

Example:

Existing Quantity: 5
Incoming Quantity: 3
New Available Quantity: 8

IMPORTANT:

Different incoming batches may have different purchase costs.

Preserve historical purchase costs.

Use a consistent inventory costing method, preferably FIFO, to calculate the cost of sold units accurately.

Do not overwrite previous purchase costs when new stock arrives at a different price.

Validate all inputs.

==================================================
6. STOCK OUT
==================================================

Implement a simple outgoing inventory workflow.

The user enters an Item Code.

The system automatically displays:

- Product Name.
- Available Quantity.

The user then enters:

- Quantity to Remove.
- Customer CR Number.
- Actual Selling Price per Unit.

CR and selling price are mandatory.

Example:

Item Code: 11234
Product: iPhone 17 Pro Max LCD
Available Quantity: 5
Outgoing Quantity: 1
CR Number: 99876
Selling Price: OMR 50.000

When Save is clicked:

1. Validate the Item Code.
2. Validate available stock.
3. Validate outgoing quantity.
4. Validate CR.
5. Validate selling price.
6. Calculate purchase cost.
7. Calculate revenue.
8. Calculate gross profit.
9. Save the transaction.
10. Deduct quantity from stock.

VALIDATION RULES:

- CR must be entered.
- CR must be a valid nonzero numeric reference.
- Selling price must be entered and nonnegative.
- Quantity must be a positive integer.
- Quantity cannot exceed available stock.

If validation fails:

- Display an error.
- Do not save the transaction.
- Do not deduct inventory.
- Preserve form input so the user can correct it.

Use atomic SQLite transactions.

==================================================
7. CR NUMBER HANDLING
==================================================

CR is the customer's external repair receipt number.

It is generated by another system.

Our application must not generate CR numbers.

Our application must not communicate with the external repair system.

CR is only a reference stored with inventory sales.

Important:

One CR may contain multiple spare parts.

Example:

CR: 99876

Item Code: 11234
Product: iPhone LCD
Quantity: 1
Selling Price: OMR 50

Item Code: 11235
Product: iPhone Battery
Quantity: 1
Selling Price: OMR 15

Item Code: 11236
Product: Charging Port
Quantity: 2
Selling Price: OMR 10 per unit

Total Sale Value: OMR 85

Requirements:

- Support multiple product lines under one CR.
- Support different quantities for each product.
- Support different selling prices for each product.
- Calculate total revenue.
- Calculate total purchase cost.
- Calculate total gross profit.
- Validate all lines before saving.
- Save all lines atomically.
- Do not partially deduct inventory.
- Allow the same CR to appear in later transactions.

Do not implement repair-order management.

Do not implement customer profiles.

Do not implement invoicing.

Do not implement CRM.

==================================================
8. AUTOMATIC PROFIT CALCULATION
==================================================

The application must calculate:

- Purchase Cost of Sold Items.
- Total Sales Revenue.
- Gross Profit.

Formula:

Gross Profit = Sales Revenue - Cost of Sold Items

Example:

Purchase Cost: OMR 5
Selling Price: OMR 15
Gross Profit: OMR 10

For multiple units, calculate totals correctly.

Use the selected stock costing method consistently.

Use OMR with three decimal places.

Store monetary values using precise decimal or integer-based representations.

Avoid floating-point rounding errors.

Do not add unnecessary accounting functionality.

==================================================
9. LOW STOCK ALERTS
==================================================

Display low-stock notifications on the dashboard.

Thresholds:

Quantity 2:
Low Stock

Quantity 1:
Critical Stock

Quantity 0:
Out of Stock

Alerts must update automatically when inventory changes.

Display the affected products.

Do not implement email alerts, push notifications, or automatic purchasing.

==================================================
10. DASHBOARD
==================================================

Create a simple, professional dashboard.

Display:

- Total Product Types.
- Total Available Units.
- Current Inventory Value.
- Monthly Sales Revenue.
- Monthly Gross Profit.
- Low Stock Alerts.

Main navigation:

1. Dashboard
2. Products / Inventory
3. Stock Entry
4. Stock Out
5. Reports
6. AI Assistant

Avoid unnecessary pages and complex analytics.

==================================================
11. PDF AND EXCEL REPORTS
==================================================

Both PDF and Excel exports are mandatory.

Users must be able to select:

Export PDF
Export Excel (.xlsx)

Required reports:

A. MONTHLY SALES REPORT

Include:

- Reporting Month.
- Item Code.
- Product Name.
- Quantity Sold.
- Customer CR Number.
- Actual Selling Price.
- Purchase Cost.
- Total Sales.
- Gross Profit.

Monthly totals:

- Total Units Sold.
- Total Purchase Cost of Sold Items.
- Total Sales Revenue.
- Total Gross Profit.

B. CURRENT INVENTORY REPORT

Include:

- Item Code.
- Product Name.
- Available Quantity.
- Inventory Purchase Value.
- Stock Status.

C. MONTHLY INVENTORY SUMMARY

Include:

- Product Code.
- Product Name.
- Quantity Sold During Month.
- Quantity Remaining.
- Total Purchase Cost of Sold Items.
- Total Sales Revenue.
- Total Gross Profit.

Allow users to select the reporting month.

All reports must use actual SQLite data.

Reports must be generated entirely offline.

Allow the user to select a local save location.

Do not add unrelated reporting modules.

==================================================
12. INTELLIGENT LOCAL AI CHATBOT
==================================================

The AI chatbot is a mandatory feature.

DO NOT REMOVE IT.

The current assistant behaves like a rigid intent classifier.

This must be improved.

The AI should behave as a natural, conversational assistant specialized in the inventory application.

It must understand Arabic and English.

It must understand different ways of expressing inventory-related questions.

It must maintain conversational context.

It must not reject legitimate inventory questions simply because they do not match predefined keywords.

EXAMPLES:

User:
How many screens do we have?

AI:
Returns the actual number from SQLite.

User:
How many did we sell this month?

AI:
Understands that the user is still referring to screens.

User:
Where can I find Samsung S3 screens?

AI:
Searches the product database by name or model.

User:
I don't have the Item Code.

AI:
Explains that products can also be searched by name or model.

User:
How do I add a product?

AI:
Explains the application's actual Stock Entry workflow.

User:
What is our profit this month?

AI:
Calculates the answer using actual sales data.

User:
What was sold under CR 99876?

AI:
Retrieves the associated product lines and prices.

The assistant must understand:

- Inventory questions.
- Product searches.
- Sales questions.
- Profit questions.
- CR-related questions.
- Stock history questions.
- Application usage questions.
- Conversational follow-ups.
- Clarification requests.

Only reject genuinely unrelated questions.

Do not return generic refusal messages for valid questions.

==================================================
13. AI DATABASE ACCESS
==================================================

The AI must have comprehensive read access to all relevant business information.

It must understand relationships between:

- Products.
- Stock Entries.
- Purchase Costs.
- Stock Out Transactions.
- CR Numbers.
- Sales.
- Profits.
- Inventory Quantities.
- Monthly Reports.

However:

Do not allow unrestricted SQL execution by the LLM.

Do not give the model arbitrary operating-system access.

Use approved backend database functions.

Examples:

get_inventory_summary()
search_products()
get_product_details()
get_product_quantity()
get_stock_history()
get_cr_transactions()
get_monthly_sales()
get_monthly_profit()
get_inventory_value()
get_low_stock_items()

The AI may combine multiple queries to answer complex questions.

All factual answers must be grounded in actual SQLite data.

Never invent stock quantities, prices, profits, or CR records.

The AI must not modify inventory through conversation.

If the user asks to add or remove stock, explain how to do it through the application interface.

==================================================
14. IMPROVE AI MODEL QUALITY
==================================================

The previous implementation uses:

Qwen3-0.6B

This model may be too weak for natural Arabic conversations and contextual reasoning.

Evaluate stronger local models when appropriate.

However, the customer's target computer is:

MacBook Intel Core i5

Therefore, AI performance and hardware compatibility are critical.

Consider:

- CPU performance.
- RAM usage.
- Model quantization.
- Context length.
- Inference latency.
- Arabic understanding.
- English understanding.
- Voice response latency.

Do not automatically select the largest available model.

Select the strongest practical model that performs reliably on the target hardware.

Test multiple suitable model sizes if necessary.

Do not assume the customer's MacBook has 16 GB RAM or a dedicated GPU.

The exact hardware specifications are still unknown.

If necessary, use a smaller optimized model for the customer's Mac while retaining an appropriate configuration for development on Windows.

The application must remain responsive while AI inference is running.

==================================================
15. AUTOMATIC AI STARTUP
==================================================

This is a mandatory fix.

Currently, every time the application opens, I must manually configure or start the AI model.

This must be eliminated.

EXPECTED BEHAVIOR:

1. User launches the application.
2. Dashboard opens immediately.
3. SQLite initializes.
4. The local AI runtime automatically starts in the background.
5. The model loads automatically.
6. The chatbot becomes available when initialization finishes.

No manual configuration should be required during ordinary use.

REMOVE the need to repeatedly:

- Select llama.cpp executable.
- Enter model file paths.
- Click Save & Start Model.
- Configure AI settings.

Automatically detect bundled AI resources.

Persist necessary settings.

Use platform-appropriate resource paths.

Never rely on hardcoded development directories.

Display:

Starting AI...
AI Ready
AI Unavailable

If the AI process crashes, attempt controlled automatic recovery.

Do not block inventory operations during AI startup.

Shut down application-owned AI processes correctly when the application closes.

Do not terminate unrelated processes.

Verify automatic startup in the installed application, not only development mode.

==================================================
16. FULL TWO-WAY AI VOICE CONVERSATION
==================================================

This is one of the most important requirements.

The current voice implementation is incomplete.

I do not want only speech-to-text.

I want a real conversational voice assistant similar in interaction style to ChatGPT Voice Mode.

The user must be able to:

- Speak to the AI.
- Have the AI understand the spoken question.
- Hear the AI respond using an actual generated voice.
- Continue speaking naturally.
- Ask follow-up questions without restarting the conversation.

VOICE WORKFLOW:

User speaks
     |
     v
Offline Speech Recognition
     |
     v
Local LLM
     |
     v
Approved Database Tools
     |
     v
Response Generation
     |
     v
Offline Text-to-Speech
     |
     v
AI Speaks Through Speakers

REQUIREMENTS:

1. Add a dedicated Voice Mode interface.
2. Include Start Conversation and End Conversation controls.
3. Automatically activate microphone listening when Voice Mode starts.
4. Detect when the user finishes speaking.
5. Convert speech to text locally.
6. Process the question through the AI assistant.
7. Retrieve database information when required.
8. Generate a natural response.
9. Convert the response into audible speech locally.
10. Play the response through the computer speakers.
11. Allow continuous conversational turns.
12. Preserve conversational context.
13. Prevent the AI from repeatedly hearing its own voice.
14. Support Arabic and English speech.
15. Support Arabic and English spoken responses.
16. Handle microphone permissions correctly.
17. Start required voice components automatically when Voice Mode is activated.
18. Do not require repeated manual configuration.
19. Keep voice completely offline.
20. Do not use paid speech APIs.

Evaluate suitable open-source components for:

- Voice Activity Detection.
- Speech-to-Text.
- Text-to-Speech.
- Audio playback and recording.

Reuse existing voice code when practical.

Do not require the customer to install Python manually.

Bundle or package all necessary voice runtime dependencies.

IMPORTANT:

Prioritize understandable Arabic and English speech with acceptable latency.

Do not claim ChatGPT-level voice performance without testing.

The customer's Intel i5 hardware may limit performance.

Optimize model sizes and resource usage accordingly.

==================================================
17. REMOVE ALL UNNECESSARY FEATURES
==================================================

Perform a full repository audit.

Remove features that are outside the confirmed customer requirements.

REMOVE:

- Username/password login.
- Multiple users.
- Administrator and employee roles.
- Permission management.
- Barcode scanning.
- QR scanning.
- Repair order management.
- Customer management.
- Supplier management.
- Storage locations.
- Shelves and boxes.
- Invoice generation.
- Payment processing.
- Appointment management.
- CRM.
- E-commerce.
- Cloud synchronization.
- Online databases.
- Paid AI services.
- Unnecessary application settings.
- Unrelated reports.
- Redundant navigation pages.

Do not remove:

- Inventory Management.
- Product Codes.
- Stock Entry.
- Stock Out.
- CR Tracking.
- Profit Calculation.
- Low Stock Alerts.
- Dashboard.
- PDF Export.
- Excel Export.
- AI Chatbot.
- Two-Way Voice Mode.
- Local SQLite Database.
- Necessary database backups and technical infrastructure.

Remove unused dependencies and obsolete code.

Do not leave broken routes or interface elements.

Before removing database tables, inspect existing records and preserve business data through safe migrations.

==================================================
18. DATABASE REVIEW AND MIGRATION
==================================================

Review the entire SQLite schema.

Ensure it supports:

PRODUCTS:
- Item Code.
- Product Name.
- Product Model / Description.

STOCK ENTRIES:
- Product Reference.
- Quantity.
- Unit Purchase Cost.
- Entry Date.

STOCK OUT TRANSACTIONS:
- CR Number.
- Transaction Date.
- Total Sales.
- Total Purchase Cost.
- Gross Profit.

STOCK OUT LINES:
- Product Reference.
- Quantity Sold.
- Actual Unit Selling Price.
- Cost of Sold Units.
- Gross Profit.

Ensure:

- Correct foreign keys.
- Accurate stock quantities.
- Transaction consistency.
- No duplicate product codes.
- No negative stock.
- Accurate profit calculations.
- Historical cost preservation.
- Safe database migrations.

Keep the SQLite database inside the application's persistent local data directory.

Keep local backup and restore functionality.

Do not delete existing customer inventory records.

==================================================
19. USER INTERFACE
==================================================

The interface must be simple enough for a nontechnical repair-center employee.

Main navigation:

Dashboard
Products
Stock Entry
Stock Out
Reports
AI Assistant

Remove all unnecessary menu items.

Use clear forms and validation messages.

Make Stock Entry and Stock Out fast and easy to use.

Use OMR currency formatting.

The dashboard should display important inventory information immediately.

Avoid unnecessary charts and complicated settings.

The AI chatbot should be accessible from its own page.

Voice Mode should be accessible directly from the AI Assistant.

==================================================
20. WINDOWS DEVELOPMENT AND TESTING
==================================================

Complete Windows development first.

Verify:

- Application launches.
- No login is required.
- Products can be created.
- Stock Entry works.
- Stock Out works.
- CR validation works.
- Multiple products can be associated with one CR.
- Profit calculations are correct.
- Low-stock alerts work.
- PDF reports work.
- Excel reports work.
- AI chatbot retrieves actual database information.
- AI understands Arabic and English.
- AI maintains conversational context.
- AI starts automatically.
- Voice Mode captures speech.
- Voice Mode produces audible responses.
- Continuous voice conversations work.
- SQLite data persists after restarting.
- Application works offline.

Build a Windows EXE installer.

Test the installed application, not only Tauri development mode.

All necessary AI and voice resources must be included.

==================================================
21. MACOS PREPARATION
==================================================

After the Windows implementation has been completed and tested:

Prepare a separate macOS build for Intel x86_64.

Requirements:

- Keep the same application features.
- Use macOS-compatible llama.cpp binaries.
- Use compatible local STT and TTS runtimes.
- Handle macOS microphone permissions.
- Use macOS application data directories.
- Generate a .app bundle.
- Generate a .dmg package where supported.
- Ensure all runtime components operate offline.

The MacBook has an Intel Core i5 processor.

Do not assume high AI processing performance.

Choose models that can realistically operate on the customer's hardware.

A macOS VM may be used for preliminary compatibility testing where supported and properly licensed.

However, virtual machine performance is not representative of the physical MacBook.

Final compatibility and AI performance must be verified on the customer's actual MacBook before deployment.

Do not falsely report successful macOS testing if no macOS environment is available.

==================================================
22. FINAL QUALITY ASSURANCE
==================================================

After implementing the changes, perform a complete double-check of the entire system.

Review:

- Frontend.
- Backend.
- SQLite schema.
- Inventory calculations.
- Stock transactions.
- CR handling.
- Profit calculations.
- Reports.
- AI chatbot.
- Conversational context.
- Voice assistant.
- Automatic AI initialization.
- Platform compatibility.
- Offline functionality.
- Application packaging.

Compare every application screen and function against these requirements.

Remove everything outside the agreed scope.

Ensure no deleted feature leaves broken dependencies or errors.

Run all automated tests.

Add tests for missing critical functionality.

Fix all build errors.

Do not mark tasks completed unless they have actually been implemented and verified.

==================================================
23. FINAL DELIVERABLES
==================================================

Provide:

1. Updated complete source code.
2. Working Windows application.
3. Windows EXE installer.
4. Corrected SQLite database schema.
5. Reliable Stock Entry and Stock Out.
6. CR-linked sales tracking.
7. Accurate purchase cost and profit calculations.
8. Low-stock alerts.
9. PDF and Excel reports.
10. Intelligent local AI chatbot.
11. Automatic local AI startup.
12. Fully functional two-way AI Voice Mode.
13. Offline runtime dependencies and models.
14. macOS-compatible source code.
15. macOS Intel build when a suitable build environment is available.
16. Installation instructions.
17. Basic user guide.
18. Test results.
19. A list of any remaining limitations or hardware-dependent issues.

==================================================
24. FINAL INSTRUCTIONS
==================================================

Continue working autonomously on the existing project.

Do not restart development from scratch.

Do not introduce features outside this specification.

Preserve all working functionality that satisfies the customer requirements.

Remove obsolete features.

Complete the Windows application first.

Then prepare the macOS version.

Optimize the local AI and voice system for the customer's Intel Core i5 MacBook.

Ensure that AI starts automatically and that voice interaction supports actual spoken conversations in both directions.

Keep the application entirely offline.

Double-check all functionality before declaring completion.

If any requirement cannot be completed or tested, clearly report the reason and the remaining work.

FINAL OBJECTIVE:

Deliver a simple, reliable, professional offline Inventory Management System for Al Mohtarif Repair Center.

The application must provide:

- Numeric Item Codes.
- Manual Stock Entry.
- Manual Stock Out.
- Mandatory CR Numbers.
- Multiple Parts per CR.
- Automatic Profit Calculations.
- Low Stock Alerts.
- PDF and Excel Reports.
- Intelligent Local AI Chatbot.
- Automatic AI Startup.
- Continuous Two-Way AI Voice Conversation.
- Local SQLite Storage.
- Windows Support.
- Future Intel macOS Deployment.

Nothing outside these requirements should remain in the customer-facing application.