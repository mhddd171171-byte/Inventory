# Google Drive backup — developer setup

The inventory database, AI, voice and local automatic backups remain local.
Google Drive is optional. No user Google password is collected by the app.
Customers use **Settings → Google Drive backup → Connect Google Drive**.

## One-time Google configuration

1. Sign in to https://console.cloud.google.com/ using the account that will own the application project.
2. Create a project named **Inventory Manager**. Enable **Google Drive API** in APIs & Services → Library.
3. In **Google Auth Platform → Branding**, enter the app name and your own support/contact email.
4. In **Audience**, choose **External**. Add your own Google email as a test user during development.
5. In **Data Access**, request only `https://www.googleapis.com/auth/drive.file`.
6. In **Clients → Create client**, choose **Desktop app**, name it Inventory Manager Desktop, and download its JSON.
7. Save that file as `C:\Users\mhdal\Downloads\inventory-google-oauth.json` for this development session. Do not put user access/refresh tokens or a service-account key in this file.
8. Test real Google sign-in and a backup in a disposable test device folder. Before customer distribution, review **Audience → Publishing status**, publish appropriately, and complete any Google-required branding verification. External apps left in Testing can have seven-day refresh-token expiry; testing-only sign-in is not a production setup.

The desktop client configuration is shared by the Windows and Intel Mac builds.
Customers do not create Google Cloud projects, install Python, or configure OAuth credentials.
An installed desktop OAuth client's client ID/client secret are public application configuration, not a user's sign-in credentials. PKCE protects the authorization exchange.

## Build input

Set `INVENTORY_GOOGLE_OAUTH_JSON` to the downloaded Desktop OAuth JSON path.
Set `INVENTORY_REQUIRE_GOOGLE_DRIVE=1` when creating the customer release so a missing configuration fails the build instead of producing a non-working Connect button.
The build reads only `installed.client_id` and `installed.client_secret`; authorization/token endpoints are fixed to Google HTTPS endpoints.
Never commit `src-tauri/google-oauth-client.json`, refresh tokens, or customer databases.
Native unit tests intentionally permit a missing public client and use a mock Google API. Those tests do not prove live OAuth registration works.

## Operation and data

- Google sign-in uses the default external browser, random loopback port, unpredictable state and PKCE S256.
- User refresh tokens go to Windows Credential Manager / macOS Keychain. They are excluded from JSON settings, database backups, reports and logs.
- A completed SQLite snapshot is copied into a durable local upload queue. The live database is never placed in a synchronized Drive folder.
- When enabled, uploads start after one quiet minute, or within ten minutes during continuous entry, while the app is open. Offline retries use bounded backoff; **Retry Drive backup** bypasses the wait.
- The app creates **Inventory Manager Backups**, then a separate **Windows/Mac - device ID** subfolder. Backup file IDs are pre-generated and persisted before upload to safely retry uncertain responses.
- Uploads are verified against the snapshot size and SHA-256 (or by hashing the downloaded bytes if Google omits that checksum).
- The latest 30 versions plus one per day for 30 backup days are retained. Cleanup only trashes this app/device's older backup files in its folder; it never permanently deletes them or touches other Drive files.
- Missing-live-database recovery and unsuccessful local backups pause uploading so an empty or stale snapshot is not described as current.
- Pause and Disconnect stop future uploads. Disconnect removes this computer's saved credential; existing Drive files remain.
- A running upload may finish its current HTTP request when paused, disconnected, or closing; no further requests are started. Pending work is retained for the next launch.
- To restore, download the desired `.db` from Drive and use the existing **Import database (.db)** flow. Restore replaces current records and saves a recovery copy first.
- Backup is not bidirectional synchronization. Restoring inventory does not copy another computer's Google credentials or device ID.

## Required release verification

Automated tests cover snapshots, queue restart/coalescing, offline/auth/storage failures, upload identity and checksum, retention boundaries, callback state and PKCE, secure-store round trips, and Settings controls without inventory writes.
Live sign-in, upload/list/download and restore of a test snapshot still require the registered client and an explicit Google account consent.
Do not describe the Google integration as production verified until those checks are complete.

References: [Google desktop OAuth](https://developers.google.com/identity/protocols/oauth2/native-app), [Drive file scope](https://developers.google.com/workspace/drive/api/guides/api-specific-auth), [resumable uploads](https://developers.google.com/workspace/drive/api/guides/manage-uploads), [SQLite snapshots](https://www.sqlite.org/backup.html).
