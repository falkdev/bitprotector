# Project Requirements Specification

## Distributed File Mirror and Integrity Protection System

---

## 1. Requirement Classification

All statements in this document are **strict requirements**.  
They must be implemented exactly as described.  
No requirement should be interpreted as optional unless explicitly stated.

This document defines the complete system requirements for implementation by AI development agents.

---

## 2. System Purpose

The system **must provide a storage protection system designed for long-term data preservation**.

The system must:

- Mirror data across multiple drives
- Protect files against **bit decay / silent corruption**
- Provide a **virtual folder structure** for files located across multiple drives
- Ensure that **each file has a mirrored copy on another drive**
- Provide both:
  - **Web frontend**
  - **CLI backend interface**
- Provide an **API used by both CLI and frontend**

---

## 3. Database Requirements

The system **must maintain a database containing metadata for all tracked files**.

The database must store at minimum:

- File paths
- File checksums
- File storage locations
- Mirror storage locations

The database must support:

- **Backup capability**
- Ability for users to **select which drives are used for database backups**
- Support for **multiple backup copies**

There must be **one master database file**.

The system must ensure that the **master database file is properly handled and preserved**.

---

## 4. Checksum Requirements

The system **must use BLAKE3 as the checksum algorithm**.

The checksum must be used for:

- Integrity validation
- Corruption detection
- File comparison

---

## 5. Drive Pairing Requirements

The system must support **paired drives**.

Each pair must contain:

- One **primary drive**
- One **secondary mirror drive**

Behavior requirements:

- Primary drive stores **master files**
- Secondary drive stores **mirror files**
- Each tracked file must exist on **both drives**

---

## 6. File Tracking Requirements

The system must support **tracking individual files**.

Users must be able to:

- Select files to track
- Enable integrity protection for tracked files

Tracked files must:

- Have checksums recorded
- Have mirror copies maintained
- Be included in integrity verification processes

---

## 7. Virtual Path Requirements

The system must support **virtual file paths**.

The virtual path system must:

- Provide a unified view of files stored across multiple drives
- Support **symlink-based virtual paths**

Users must be able to:

- Define virtual paths
- Map real file locations to virtual paths

---

## 8. Bulk Virtual Path Assignment Requirements

The system must support **bulk operations for assigning virtual paths**.

Users must be able to:

- Select files under folders in bulk
- Copy portions of the real path into the virtual path

---

## 9. File Mirroring Requirements

When a tracked file is created:

1. The **master file must reside on the primary drive**
2. The system must **automatically copy the file to the secondary mirror drive**

The system must ensure that both copies remain synchronized.

---

## 10. File Synchronization Requirements

The system must perform **file synchronization and integrity verification**.

These operations must:

- Run during **specified times**
- Run at **specified intervals**

Users must be able to configure these schedules.

---

## 11. Integrity Check Requirements

During integrity checks the system must detect corruption.

Integrity validation must compare:

- Master file
- Mirror file

The system must perform the following recovery actions:

### Case 1

If the **master file is corrupted** and the **mirror file is valid**:

- The system must restore the master file from the mirror.

### Case 2

If the **mirror file is corrupted** and the **master file is valid**:

- The system must restore the mirror file from the master.

### Case 3

If **both copies are corrupted**:

- The system must report the failure
- The system must require **user action**

---

## 12. Event Logging Requirements

The system must include an **application logging system**.

The logging system must record events including:

- File creation
- File edits
- Integrity verification results
- Integrity failures
- Recovery actions

Users must be able to:

- View logs through the application
- Access logs through CLI and frontend interfaces

---

## 13. Integrity Failure Handling Requirements

When an integrity failure occurs:

- The failure must be **reported**
- The system must require **user intervention**

The system must not automatically resolve failures that require user decisions.

---

## 14. File Change Detection Requirements

The system must detect when a **master file has been modified**.

When this occurs the system must:

- Record the change
- Notify the user
- Inform the user what action must be taken

---

## 15. SSH Login Status Requirements

When a user logs into the system through **SSH**, the system must display a **short status message**.

The message must notify the user if:

- Files have changed
- Integrity issues exist
- Files require handling

---

## 16. Sync Queue Requirements

Files that require handling must:

- Be automatically added to a **sync queue**

The sync queue must be processed during scheduled:

- File synchronization
- Integrity verification tasks

---

## 17. Folder Tracking Requirements

The system must support **tracked folders**.

Users must be able to configure folders whose contents are automatically tracked.

---

## 18. Default Folder Behavior Requirements

Tracked folders must support **default behavior configuration**.

Users must be able to configure behavior so that:

- Files added to tracked folders are **automatically linked to the virtual path**

---

## 19. Backend Implementation Requirements

The backend implementation must:

- Use a **memory-efficient and resource-efficient programming language**
- Operate correctly in **resource-constrained environments**

The implementation language must be selected based on **efficiency and low resource consumption**.

---

## 20. Code Documentation Requirements

All code in the project must be:

- Clearly documented
- Thoroughly explained
- Maintainable

Documentation must be written alongside implementation.

---

## 21. Development Methodology Requirements

The project must use **Test Driven Development (TDD)**.

Tests must be written before implementation.

The system must include:

- Unit tests
- Module tests
- Integration tests

---

## 22. Planning Requirements

The complete project plan must be written in **Markdown format**.

The plan must include:

- System architecture
- Implementation steps
- Development milestones

Whenever changes occur:

- The plan must be updated accordingly.

---

## 23. Version Control Requirements

The project must use **Git** for version control.

Requirements:

- Each functional milestone must be committed
- Each commit must contain an **elaborate commit message**

---

## 24. Web Server Requirements

The system must include a **web server** to support the frontend.

---

## 25. Frontend Technology Requirements

The frontend must be implemented using **React**.

---

## 26. Drive Failure and Replacement Requirements

The system must support **planned drive replacement** and **unexpected drive failure** handling.

The system must:

- Track the runtime state of each drive slot
- Support the states `active`, `quiescing`, `failed`, and `rebuilding`
- Track which slot is currently the **active role**

For planned replacement, users must be able to:

- Mark a drive slot for replacement
- Confirm the failure after external I/O has been quiesced
- Cancel the replacement workflow before confirmation
- Assign a new mounted path to the failed slot

The system must preserve:

- The logical drive pair
- Relative file paths
- Tracked file metadata
- Virtual path mappings

---

## 27. Live Failover Requirements

If the currently active drive becomes unavailable, the system must support **live failover** to the surviving slot.

The system must:

- Continue serving future file access from the surviving active slot
- Retarget virtual-path symlinks to the new active slot
- Allow reads and writes on the surviving slot while degraded
- Update stored checksum and file-size metadata from the surviving active copy
- Keep files marked as not fully mirrored until rebuild completes

The system must not claim to preserve already-open operating-system file handles after sudden disk loss.

The system must support rebuilding the failed slot from the surviving slot once replacement media is mounted.

---

## 28. Failover Test Coverage Requirements

The project must include automated test coverage for failover and replacement behavior.

The test suite must include:

- Unit tests for drive-state transitions and active/standby path resolution
- Integration tests for CLI and API replacement flows
- QEMU end-to-end tests using multiple virtual drives

The QEMU tests must cover:

- Planned primary failover
- Replacement rebuild onto a new drive
- Virtual-path retargeting during failover and after rebuild
- Emergency failover after hot-removing the active disk through a QMP control socket

---

## 26. CLI Configuration Requirements

All system functionality must be configurable through the **CLI interface**.

No functionality may be limited to the frontend.

---

## 27. Secure Communication Requirements

Communication between the frontend and backend must:

- Be encrypted
- Use secure login mechanisms

Authentication must use:

- **Local system accounts**

---

## 28. Frontend and Backend Planning Requirements

The project plan must include **two separate implementation plans**:

1. Backend implementation plan
2. Frontend implementation plan

The two plans must be separated.

The backend plan must include a **complete API specification**.

The frontend implementation will be performed by **another AI agent** and must rely on the API specification.

---

## 29. Frontend Design Requirements

The frontend must use a **file browser style interface as the main design**.

---

## 30. Packaging Requirements

The system must be packaged for **Ubuntu 24**.

Installation must be provided through a package installer.

---

## 31. Installation Testing Requirements

The installation process must include **automatic testing**.

Preferred testing method:

- QEMU virtualization
- Ubuntu cloud image

The installation test must verify:

- Package installation
- Application startup
- Basic system functionality

---
