# P2P Folder Sharing - Testing Guide

> **Feature:** Peer-to-Peer Folder Sharing with Sync
> **Last Updated:** 2026-04-01

This guide provides comprehensive testing steps for validating the P2P folder sharing feature across all implementation phases.

---

## Prerequisites

### Setup for Testing

1. **Two devices on the same network** (for Phase 1-2 testing)
   - Can be two computers, or use one computer with two separate Scratch instances
   - Ensure both devices can reach each other via network

2. **Scratch installed** on both devices with P2P feature implemented

3. **Test notes folder** prepared with sample content:
   ```bash
   # Create a test folder with sample notes
   mkdir -p ~/TestNotes/Recipes
   echo "# Pasta Carbonara

Ingredients:
- Spaghetti
- Eggs
- Pecorino Romano
- Guanciale

## Instructions

1. Cook pasta...
2. Mix eggs and cheese...
3. Combine and serve..." > ~/TestNotes/Recipes/carbonara.md
   ```

---

## Phase 1 Testing: Core LAN Sharing

### Test 1.1: P2P Initialization

**Objective:** Verify P2P networking starts correctly

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Launch Scratch on Device A | App starts normally |
| 2 | Open Settings → About | P2P status should show "Running" |
| 3 | Note the Peer ID displayed | A unique peer ID is shown (starts with `12D3KooW...`) |
| 4 | Repeat on Device B | Different peer ID is shown |

**Pass Criteria:** Both devices show running P2P status with unique peer IDs

---

### Test 1.2: Create Share (Invite Code Generation)

**Objective:** Verify invite code generation

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | On Device A, navigate to folder tree | Folder tree visible |
| 2 | Right-click on "Recipes" folder | Context menu appears |
| 3 | Select "Share Folder..." | Share modal opens |
| 4 | Verify folder name is correct | Modal shows "📁 Recipes" |
| 5 | Select "Can edit" permission | Radio button selected |
| 6 | Click "Share" button | Invite code generates |
| 7 | Verify invite code format | Code starts with `scratch-share-` |
| 8 | Click "Copy" button | Invite code copied to clipboard |
| 9 | Click "Show QR Code" | QR code displays |
| 10 | Close modal | Modal closes, share active |

**Pass Criteria:** Invite code generated successfully, format correct, QR code displays

**Invite Code Format:**
```
scratch-share-[base64-encoded-encrypted-payload]
```

---

### Test 1.3: Accept Share (QR Code)

**Objective:** Verify share acceptance via QR code

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | On Device B, open Scratch | App starts |
| 2 | Click "Enter Invite Code" in sidebar | Accept share modal opens |
| 3 | (Alternative) Scan QR code from Device A | Invite code auto-fills |
| 4 | Verify folder preview shows | Shows "Recipes from Alice" |
| 5 | Verify permission shows | "Can edit" displayed |
| 6 | Verify file count | Shows "1 note" |
| 7 | Enter destination: "Shared/Recipes" | Path entered |
| 8 | Click "Accept" button | Share acceptance begins |
| 9 | Wait for sync to complete | "Synced" status shown |
| 10 | Close modal | Folder appears in tree |

**Pass Criteria:** Share accepted, folder appears in Device B's tree

---

### Test 1.4: Accept Share (Manual Entry)

**Objective:** Verify share acceptance via manual code entry

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | On Device B, open Scratch | App starts |
| 2 | Click "Enter Invite Code" | Accept share modal opens |
| 3 | Manually paste invite code | Code entered in text field |
| 4 | Click "Parse" or continue | Folder preview loads |
| 5 | Complete Test 1.3 steps 5-10 | Share accepted |

**Pass Criteria:** Manual code entry works same as QR scan

---

### Test 1.5: Initial Sync

**Objective:** Verify files sync from sharer to receiver

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | On Device A, verify "Recipes" folder exists | Folder exists |
| 2 | On Device B, check "Shared/Recipes" | Folder exists |
| 3 | Compare files | Device B has same files as A |
| 4 | Open "carbonara.md" on Device B | Content matches Device A |

**Pass Criteria:** All files and content match between devices

---

### Test 1.6: Bidirectional Sync - New File

**Objective:** Verify new files sync in both directions

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | On Device A, create new file "tiramisu.md" | File created |
| 2 | Wait 3 seconds | Sync occurs |
| 3 | On Device B, check "Shared/Recipes" | "tiramisu.md" appears |
| 4 | On Device B, create new file "gelato.md" | File created |
| 5 | Wait 3 seconds | Sync occurs |
| 6 | On Device A, check "Recipes" | "gelato.md" appears |

**Pass Criteria:** New files sync in both directions

---

### Test 1.7: Bidirectional Sync - Edit File

**Objective:** Verify file edits sync

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | On Device A, edit "carbonara.md" | Add new ingredient |
| 2 | Save changes | Auto-saves |
| 3 | Wait 3 seconds | Sync occurs |
| 4 | On Device B, open "carbonara.md" | New ingredient visible |
| 5 | On Device B, edit same file | Add new instruction |
| 6 | Wait 3 seconds | Sync occurs |
| 7 | On Device A, reload file | New instruction visible |

**Pass Criteria:** File edits sync bidirectionally

---

### Test 1.8: Bidirectional Sync - Delete File

**Objective:** Verify file deletion syncs

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | On Device A, delete "tiramisu.md" | File removed |
| 2 | Wait 3 seconds | Sync occurs |
| 3 | On Device B, check folder | "tiramisu.md" removed |
| 4 | On Device B, delete "gelato.md" | File removed |
| 5 | Wait 3 seconds | Sync occurs |
| 6 | On Device A, check folder | "gelato.md" removed |

**Pass Criteria:** File deletions sync bidirectionally

---

### Test 1.9: Shared Folders Settings

**Objective:** Verify shared folders appear in settings

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | On Device A, open Settings → Shared Folders | Settings page opens |
| 2 | Check "Folders you're sharing" section | "Recipes" appears |
| 3 | Verify peer info shows | "Shared with: Bob" |
| 4 | Check sync status | "● Synced 2m ago" |
| 5 | On Device B, open Settings → Shared Folders | Settings page opens |
| 6 | Check "Folders shared with you" section | "Recipes" appears |
| 7 | Verify peer info shows | "From: Alice" |
| 8 | Check sync status | "● Synced 2m ago" |

**Pass Criteria:** Both devices show shared folders correctly

---

## Phase 2 Testing: Enhanced Sharing

### Test 2.1: Revoke Share

**Objective:** Verify share revocation

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | On Device A, open Settings → Shared Folders | Settings page opens |
| 2 | Find "Recipes" in sharing list | Share listed |
| 3 | Click "Revoke" button | Confirmation dialog appears |
| 4 | Confirm revocation | Share revoked |
| 5 | On Device B, wait 5 seconds | Sync status changes |
| 6 | Check sync status on Device B | Shows "Share revoked" or "Offline" |
| 7 | Try to edit file on Device B | "Permission denied" error |
| 8 | On Device A, verify share removed | No longer in sharing list |

**Pass Criteria:** Share revoked, Device B loses access

---

### Test 2.2: Leave Share

**Objective:** Verify leaving a share

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | On Device B, open Settings → Shared Folders | Settings page opens |
| 2 | Find "Recipes" in "shared with you" | Share listed |
| 3 | Click "Leave" button | Confirmation dialog appears |
| 4 | Confirm leaving | Share left |
| 5 | On Device B, verify folder removed | Folder no longer in tree |
| 6 | On Device A, check settings | Share still active for A |

**Pass Criteria:** Device B leaves share, Device A unaffected

---

### Test 2.3: Manual Sync Trigger

**Objective:** Verify manual sync functionality

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | On Device A, disable network | Network offline |
| 2 | Edit "carbonara.md" on Device A | File edited |
| 3 | Re-enable network | Network back online |
| 4 | On Device A, open Settings → Shared Folders | Settings opens |
| 5 | Find "Recipes", click "Sync Now" | Manual sync starts |
| 6 | Wait for sync to complete | Status shows "Synced" |
| 7 | On Device B, check file | Changes appear |

**Pass Criteria:** Manual sync triggers successfully

---

### Test 2.4: Conflict Detection - Same File Edit

**Objective:** Verify conflict detection when both edit same file

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | On Device A, disconnect from network | Network offline |
| 2 | On Device B, disconnect from network | Network offline |
| 3 | On Device A, edit "carbonara.md" - add line "Test A" | File edited |
| 4 | On Device B, edit "carbonara.md" - add line "Test B" | File edited |
| 5 | Reconnect Device A to network | Online |
| 6 | Reconnect Device B to network | Online |
| 7 | Wait 5 seconds | Conflict detected |
| 8 | Verify conflict dialog appears | Shows both versions |
| 9 | Select "Keep Both" | Both versions saved |

**Pass Criteria:** Conflict detected, resolution options available

---

### Test 2.5: Conflict Resolution - Keep Local

**Objective:** Verify "Keep Local" conflict resolution

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Create conflict (as in Test 2.4) | Conflict exists |
| 2 | In conflict dialog, click "Keep Local" | Local version kept |
| 3 | Verify file content | Contains "Test A" (Device A content) |
| 4 | On Device B, check file | "Test A" content synced |
| 5 | Verify no conflict copy created | Only one file exists |

**Pass Criteria:** Local version kept, synced to peer

---

### Test 2.6: Conflict Resolution - Keep Remote

**Objective:** Verify "Keep Remote" conflict resolution

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Create conflict (as in Test 2.4) | Conflict exists |
| 2 | In conflict dialog, click "Keep Remote" | Remote version kept |
| 3 | Verify file content | Contains "Test B" (Device B content) |
| 4 | On Device B, check file | "Test B" content present |

**Pass Criteria:** Remote version kept

---

### Test 2.7: Conflict Resolution - Keep Both

**Objective:** Verify "Keep Both" creates conflict copy

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Create conflict (as in Test 2.4) | Conflict exists |
| 2 | In conflict dialog, click "Keep Both" | Both versions saved |
| 3 | Verify original file | Contains "Test A" |
| 4 | Verify conflict copy exists | File named "carbonara (conflict copy).md" |
| 5 | Open conflict copy | Contains "Test B" |
| 6 | On Device B, verify both files | Both synced to B |

**Pass Criteria:** Both versions preserved with conflict copy

---

### Test 2.8: Sync Status Indicator

**Objective:** Verify sync status indicator shows correct states

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | On Device A, observe Shared Folders | Status visible |
| 2 | Check status indicator | Green dot (Synced) |
| 3 | Edit a file | Status changes to yellow |
| 4 | Wait for sync | Status returns to green |
| 5 | Disconnect network | Status shows gray (Offline) |
| 6 | Reconnect network | Status shows yellow, then green |

**Pass Criteria:** Status indicator reflects actual sync state

---

### Test 2.9: Read-Only Permission

**Objective:** Verify read-only permission enforcement

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | On Device A, create new share for "Desserts" | Share created |
| 2 | Set permission to "View only" | Read-only selected |
| 3 | Generate invite code | Code generated |
| 4 | On Device B, accept share | Share accepted |
| 5 | Try to edit file on Device B | "Read-only" error |
| 6 | Try to create file on Device B | "Read-only" error |
| 7 | On Device A, edit file | Edit successful |
| 8 | On Device B, verify change | Change synced (read-only) |

**Pass Criteria:** Read-only enforced, receiver cannot edit

---

## Phase 3 Testing: WAN + Advanced Features

### Test 3.1: WAN Connection - Different Networks

**Objective:** Verify sync works across different networks

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | On Device A (home network), create share | Share active |
| 2 | Generate invite code | Code generated |
| 3 | Send invite to Device B (work network) | Invite received |
| 4 | On Device B, accept share | Connection attempts |
| 5 | Wait for WebRTC connection | Connection establishes |
| 6 | Verify sync status | Shows "Synced" |
| 7 | Edit file on Device A | Change syncs to B |
| 8 | Edit file on Device B | Change syncs to A |

**Pass Criteria:** WAN sync functional via WebRTC

---

### Test 3.2: Relay Fallback (Restrictive NAT)

**Objective:** Verify relay fallback for restrictive NAT

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Configure Device A behind restrictive NAT | NAT blocks direct |
| 2 | On Device A, create share | Share active |
| 3 | On Device B, accept share | Direct connection fails |
| 4 | Verify relay connection established | Status shows "Via relay" |
| 5 | Test file sync | Sync works via relay |
| 6 | Verify performance | Slightly slower but functional |

**Pass Criteria:** Relay fallback works when direct connection fails

---

### Test 3.3: Multi-Peer Sharing

**Objective:** Verify sharing folder with multiple people

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | On Device A, create share for "Recipes" | Share active |
| 2 | Generate invite for Device B | Invite 1 created |
| 3 | Generate invite for Device C | Invite 2 created |
| 4 | Device B accepts share | B connected |
| 5 | Device C accepts share | C connected |
| 6 | On Device A, check settings | Shows 2 peers connected |
| 7 | Edit file on Device A | Changes sync to B and C |
| 8 | Device B edits file | Changes sync to A and C |
| 9 | Device C edits file | Changes sync to A and B |

**Pass Criteria:** Multi-peer sharing functional

---

### Test 3.4: Large File Sync

**Objective:** Verify large files sync correctly

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Create 50 MB file in shared folder | File created |
| 2 | Wait for sync | Sync in progress |
| 3 | Check sync progress indicator | Progress shows |
| 4 | Wait for completion | Sync completes |
| 5 | On Device B, verify file | File identical |
| 6 | Compare file hashes | Hashes match |

**Pass Criteria:** Large files sync successfully

---

### Test 3.5: Many Files Sync

**Objective:** Verify sync with many files

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Create 100+ files in shared folder | Files created |
| 2 | Wait for sync | Sync processes all files |
| 3 | On Device B, verify file count | All 100+ files present |
| 4 | Verify file contents | All files intact |

**Pass Criteria:** Bulk sync handles 100+ files

---

### Test 3.6: Network Interruption During Sync

**Objective:** Verify sync resumes after network interruption

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Start syncing large file (50 MB) | Sync in progress |
| 2 | Disconnect network at 50% | Sync pauses |
| 3 | Reconnect network | Sync resumes |
| 4 | Verify file completes | File fully synced |
| 5 | On Device B, verify file integrity | File complete and valid |

**Pass Criteria:** Sync resumes and completes after interruption

---

### Test 3.7: Activity Log

**Objective:** Verify activity log tracks sync events

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | On Device A, open Activity Log | Log visible |
| 2 | Edit a file | Event logged |
| 3 | Check log | Shows "Edited carbonara.md" |
| 4 | Device B edits file | Event logged |
| 5 | Check log | Shows "Bob edited pasta.md" |
| 6 | Verify timestamps | Accurate timestamps |

**Pass Criteria:** Activity log tracks all sync events

---

## Edge Cases & Negative Testing

### Test EC.1: Invalid Invite Code

**Objective:** Verify invalid invite handling

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | On Device B, click "Enter Invite Code" | Modal opens |
| 2 | Enter invalid code: "invalid-code" | Submit |
| 3 | Verify error message | "Invalid invite code" shown |
| 4 | Verify no share created | No folder added |

**Pass Criteria:** Invalid codes rejected with error

---

### Test EC.2: Expired Invite Code

**Objective:** Verify expired invite handling

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Create share with 24-hour expiration | Share created |
| 2 | Wait 25 hours (or modify timestamp) | Invite expired |
| 3 | Try to accept with expired code | Attempt fails |
| 4 | Verify error message | "Invite code expired" shown |

**Pass Criteria:** Expired codes rejected

---

### Test EC.3: Path Traversal Attack

**Objective:** Verify path traversal protection

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Create share for "Recipes" | Share active |
| 2 | On Device B, attempt to access file with `../../etc/passwd` | Attempt blocked |
| 3 | Verify error message | "Path traversal detected" |
| 4 | Verify no unauthorized access | Access denied |

**Pass Criteria:** Path traversal attacks blocked

---

### Test EC.4: File Too Large

**Objective:** Verify file size limit enforcement

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Create 150 MB file in shared folder | File exceeds limit |
| 2 | Wait for sync attempt | Sync tries |
| 3 | Verify error message | "File too large (max 100 MB)" |
| 4 | Verify file not synced | File not transferred |

**Pass Criteria:** Oversized files rejected

---

### Test EC.5: Simultaneous Multi-File Conflicts

**Objective:** Verify handling of multiple conflicts

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Create conflicts in 5 files simultaneously | Multiple conflicts |
| 2 | Wait for sync | All conflicts detected |
| 3 | Verify conflict dialog | Lists all 5 conflicts |
| 4 | Resolve each conflict | Can resolve individually |
| 5 | Select "Resolve All" | All conflicts resolved |

**Pass Criteria:** Multiple conflicts handled correctly

---

### Test EC.6: Rename Shared Folder

**Objective:** Verify behavior when folder renamed

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Share "Recipes" folder | Share active |
| 2 | Rename folder to "Cooking" | Folder renamed |
| 3 | Check share status | Share still active |
| 4 | On Device B, verify sync | Sync continues |
| 5 | Verify share still works | Files still sync |

**Pass Criteria:** Folder rename doesn't break share

---

### Test EC.7: Delete Shared Folder

**Objective:** Verify behavior when folder deleted

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Share "Recipes" folder | Share active |
| 2 | Delete "Recipes" folder | Folder removed |
| 3 | Check share status | Share marked as "Folder deleted" |
| 4 | On Device B, verify behavior | Files remain on B |
| 5 | Check share settings | Option to recreate share |

**Pass Criteria:** Graceful handling of folder deletion

---

## Performance Testing

### Test Perf.1: Sync Speed

**Objective:** Measure sync performance

| File Size | Expected Sync Time | Actual | Pass/Fail |
|-----------|-------------------|--------|-----------|
| 1 MB | < 2 seconds | ___ | ___ |
| 10 MB | < 10 seconds | ___ | ___ |
| 50 MB | < 60 seconds | ___ | ___ |

---

### Test Perf.2: Concurrent Transfers

**Objective:** Verify concurrent file transfer limit

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Create 10 new files simultaneously | Files created |
| 2 | Monitor active transfers | Max 3 concurrent |
| 3 | Wait for completion | All files sync |
| 4 | Verify no errors | Clean sync |

**Pass Criteria:** Max 3 concurrent transfers, all complete

---

### Test Perf.3: Memory Usage

**Objective:** Verify memory usage during sync

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Start syncing 100 files | Sync begins |
| 2 | Monitor memory usage | < 500 MB |
| 3 | Let sync complete | Sync finishes |
| 4 | Check for memory leaks | Memory returns to baseline |

**Pass Criteria:** Memory usage reasonable, no leaks

---

## Security Testing

### Test Sec.1: Encryption Verification

**Objective:** Verify traffic is encrypted

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Start Wireshark or similar | Network capture ready |
| 2 | Create share and sync files | Traffic generated |
| 3 | Analyze captured packets | No plaintext content visible |
| 4 | Verify Noise protocol | Encrypted packets only |

**Pass Criteria:** All traffic encrypted

---

### Test Sec.2: Peer Authentication

**Objective:** Verify peer authentication

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Capture Device A's peer ID | ID: 12D3KooW... |
| 2 | Attempt to impersonate with different ID | Connection rejected |
| 3 | Verify authentication failure | "Peer authentication failed" |

**Pass Criteria:** Impersonation attempts rejected

---

## Test Results Summary

### Phase 1: Core LAN Sharing

| Test | Status | Notes |
|------|--------|-------|
| 1.1: P2P Initialization | ⬜ Pass / ❌ Fail | |
| 1.2: Create Share | ⬜ Pass / ❌ Fail | |
| 1.3: Accept Share (QR) | ⬜ Pass / ❌ Fail | |
| 1.4: Accept Share (Manual) | ⬜ Pass / ❌ Fail | |
| 1.5: Initial Sync | ⬜ Pass / ❌ Fail | |
| 1.6: Bidirectional - New File | ⬜ Pass / ❌ Fail | |
| 1.7: Bidirectional - Edit | ⬜ Pass / ❌ Fail | |
| 1.8: Bidirectional - Delete | ⬜ Pass / ❌ Fail | |
| 1.9: Shared Folders Settings | ⬜ Pass / ❌ Fail | |

### Phase 2: Enhanced Sharing

| Test | Status | Notes |
|------|--------|-------|
| 2.1: Revoke Share | ⬜ Pass / ❌ Fail | |
| 2.2: Leave Share | ⬜ Pass / ❌ Fail | |
| 2.3: Manual Sync | ⬜ Pass / ❌ Fail | |
| 2.4: Conflict Detection | ⬜ Pass / ❌ Fail | |
| 2.5: Conflict - Keep Local | ⬜ Pass / ❌ Fail | |
| 2.6: Conflict - Keep Remote | ⬜ Pass / ❌ Fail | |
| 2.7: Conflict - Keep Both | ⬜ Pass / ❌ Fail | |
| 2.8: Sync Status Indicator | ⬜ Pass / ❌ Fail | |
| 2.9: Read-Only Permission | ⬜ Pass / ❌ Fail | |

### Phase 3: WAN + Advanced

| Test | Status | Notes |
|------|--------|-------|
| 3.1: WAN Connection | ⬜ Pass / ❌ Fail | |
| 3.2: Relay Fallback | ⬜ Pass / ❌ Fail | |
| 3.3: Multi-Peer Sharing | ⬜ Pass / ❌ Fail | |
| 3.4: Large File Sync | ⬜ Pass / ❌ Fail | |
| 3.5: Many Files Sync | ⬜ Pass / ❌ Fail | |
| 3.6: Network Interruption | ⬜ Pass / ❌ Fail | |
| 3.7: Activity Log | ⬜ Pass / ❌ Fail | |

### Edge Cases

| Test | Status | Notes |
|------|--------|-------|
| EC.1: Invalid Invite | ⬜ Pass / ❌ Fail | |
| EC.2: Expired Invite | ⬜ Pass / ❌ Fail | |
| EC.3: Path Traversal | ⬜ Pass / ❌ Fail | |
| EC.4: File Too Large | ⬜ Pass / ❌ Fail | |
| EC.5: Multi-File Conflicts | ⬜ Pass / ❌ Fail | |
| EC.6: Rename Folder | ⬜ Pass / ❌ Fail | |
| EC.7: Delete Folder | ⬜ Pass / ❌ Fail | |

---

## Bug Report Template

If a test fails, use this template:

```markdown
### Bug Report: [Test Name]

**Test:** [Test ID and Name]
**Status:** ❌ Fail

**Steps to Reproduce:**
1.
2.
3.

**Expected Result:**
[What should happen]

**Actual Result:**
[What actually happened]

**Screenshots/Logs:**
[Attach relevant screenshots or logs]

**Environment:**
- OS: [macOS/Windows/Linux]
- Scratch Version: [x.x.x]
- Network: [LAN/WAN]
```

---

## Quick Reference Commands

### During Testing

```bash
# View P2P logs
tail -f ~/Library/Logs/scratch/p2p.log  # macOS
tail -f ~/.local/share/scratch/p2p.log  # Linux

# Check shares storage
cat ~/Notes/.scratch/shares.json

# Check sync manifests
ls ~/Notes/.scratch/sync/

# Reset all shares (for testing)
rm ~/Notes/.scratch/shares.json
```

---

**Testing Guide Status:** Ready for Use
