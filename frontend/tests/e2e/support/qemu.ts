import { randomUUID } from 'node:crypto'
import { spawn } from 'node:child_process'
import type { TestInfo } from '@playwright/test'

function shellQuote(value: string) {
  return `'${value.replace(/'/g, `'\\''`)}'`
}

function sanitizeSegment(value: string) {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 32)
}

function readEnv() {
  return {
    sshHost: process.env.QEMU_SSH_HOST ?? 'localhost',
    sshPort: process.env.QEMU_SSH_PORT ?? '2222',
    sshUser: process.env.QEMU_SSH_USER ?? 'testuser',
    webUser: process.env.QEMU_WEB_USER ?? 'testuser',
    webPassword: process.env.QEMU_WEB_PASSWORD ?? 'bitprotector',
  }
}

function sshArgs() {
  const env = readEnv()
  return [
    '-T',
    '-o',
    'StrictHostKeyChecking=no',
    '-o',
    'UserKnownHostsFile=/dev/null',
    '-o',
    'ConnectTimeout=10',
    '-p',
    env.sshPort,
    `${env.sshUser}@${env.sshHost}`,
  ]
}

function runProcess(command: string, args: string[], input?: string) {
  return new Promise<{ stdout: string; stderr: string }>((resolve, reject) => {
    const child = spawn(command, args, {
      stdio: 'pipe',
      env: process.env,
    })

    let stdout = ''
    let stderr = ''

    child.stdout.on('data', (chunk: Buffer | string) => {
      stdout += chunk.toString()
    })
    child.stderr.on('data', (chunk: Buffer | string) => {
      stderr += chunk.toString()
    })
    child.on('error', reject)
    child.on('close', (code) => {
      if (code === 0) {
        resolve({ stdout, stderr })
        return
      }

      reject(
        new Error(`${command} ${args.join(' ')} failed with exit code ${code}\n${stderr || stdout}`)
      )
    })

    if (input) {
      child.stdin.write(input)
    }
    child.stdin.end()
  })
}

async function runGuestScript(script: string) {
  return runProcess('ssh', [...sshArgs(), 'bash -seu'], script)
}

export interface SeededDriveFixture {
  runId: string
  driveName: string
  updatedDriveName: string
  primaryPath: string
  secondaryPath: string
  replacementPrimaryPath: string
  folderRelativePath: string
  fileRelativePath: string
  absoluteFilePath: string
  secondaryFilePath: string
  virtualPath: string
  folderVirtualPath: string
  backupPath: string
}

export interface QemuContext {
  runId: string
  env: ReturnType<typeof readEnv>
  seedDriveFixture(): Promise<SeededDriveFixture>
  runBitProtector(args: string[]): Promise<{ stdout: string; stderr: string }>
  resolvePath(path: string): Promise<string>
  readFile(path: string): Promise<string>
  pathExists(path: string): Promise<boolean>
  diagnostics(): Promise<string>
  cleanup(): Promise<void>
}

export function createRunId(testInfo: TestInfo) {
  const titlePart = sanitizeSegment(testInfo.title) || 'test'
  return `e2e-${testInfo.parallelIndex}-${titlePart}-${randomUUID().slice(0, 8)}`
}

export function createQemuContext(runId: string): QemuContext {
  const env = readEnv()

  const basePaths = {
    primaryRoot: `/mnt/primary/e2e/${runId}`,
    mirrorRoot: `/mnt/mirror/e2e/${runId}`,
    replacementPrimaryRoot: `/mnt/replacement-primary/e2e/${runId}`,
    spareRoot: `/mnt/spare1/e2e/${runId}`,
    virtualRoot: `/tmp/bitprotector-virtual/${runId}`,
  }

  return {
    runId,
    env,
    async seedDriveFixture() {
      const uniqueSuffix = runId.slice(-8)
      const driveName = `e2e-${runId}`
      const updatedDriveName = `${driveName}-updated`
      const primaryPath = `${basePaths.primaryRoot}/primary-drive`
      const secondaryPath = `${basePaths.mirrorRoot}/secondary-drive`
      const replacementPrimaryPath = `${basePaths.replacementPrimaryRoot}/replacement-primary`
      const folderRelativePath = `docs-${uniqueSuffix}`
      const fileName = `report-${uniqueSuffix}.txt`
      const notesFileName = `notes-${uniqueSuffix}.txt`
      const fileRelativePath = `${folderRelativePath}/${fileName}`
      const absoluteFilePath = `${primaryPath}/${fileRelativePath}`
      const secondaryFilePath = `${secondaryPath}/${fileRelativePath}`
      const backupPath = `${basePaths.spareRoot}/backups`
      const virtualPath = `${basePaths.virtualRoot}/files/${fileName}`
      const folderVirtualPath = `${basePaths.virtualRoot}/folders/${folderRelativePath}`

      await runGuestScript(`
PRIMARY_PATH=${shellQuote(primaryPath)}
SECONDARY_PATH=${shellQuote(secondaryPath)}
REPLACEMENT_PRIMARY_PATH=${shellQuote(replacementPrimaryPath)}
ABSOLUTE_FILE_PATH=${shellQuote(absoluteFilePath)}
BACKUP_PATH=${shellQuote(backupPath)}
FOLDER_RELATIVE_PATH=${shellQuote(folderRelativePath)}
NOTES_FILE_NAME=${shellQuote(notesFileName)}

mkdir -p "$PRIMARY_PATH/$FOLDER_RELATIVE_PATH" "$SECONDARY_PATH" "$REPLACEMENT_PRIMARY_PATH" "$BACKUP_PATH"
printf 'report for ${runId}\n' > "$ABSOLUTE_FILE_PATH"
printf 'notes for ${runId}\n' > "$PRIMARY_PATH/$FOLDER_RELATIVE_PATH/$NOTES_FILE_NAME"
`)

      return {
        runId,
        driveName,
        updatedDriveName,
        primaryPath,
        secondaryPath,
        replacementPrimaryPath,
        folderRelativePath,
        fileRelativePath,
        absoluteFilePath,
        secondaryFilePath,
        virtualPath,
        folderVirtualPath,
        backupPath,
      }
    },
    async runBitProtector(args: string[]) {
      return runGuestScript(
        `sudo /usr/bin/bitprotector ${args.map((arg) => shellQuote(arg)).join(' ')}`
      )
    },
    async resolvePath(path: string) {
      const { stdout } = await runGuestScript(`readlink -f ${shellQuote(path)}`)
      return stdout.trim()
    },
    async readFile(path: string) {
      const { stdout } = await runGuestScript(`cat ${shellQuote(path)}`)
      return stdout
    },
    async pathExists(path: string) {
      try {
        await runGuestScript(`test -e ${shellQuote(path)}`)
        return true
      } catch {
        return false
      }
    },
    async diagnostics() {
      const { stdout, stderr } = await runGuestScript(`
sudo systemctl status bitprotector --no-pager -l || true
echo
sudo journalctl -u bitprotector -n 80 --no-pager || true
echo
sudo /usr/bin/bitprotector status || true
`)
      return `${stdout}${stderr}`
    },
    async cleanup() {
      await runGuestScript(`
sudo rm -rf \
  ${shellQuote(basePaths.primaryRoot)} \
  ${shellQuote(basePaths.mirrorRoot)} \
  ${shellQuote(basePaths.replacementPrimaryRoot)} \
  ${shellQuote(basePaths.spareRoot)} \
  ${shellQuote(basePaths.virtualRoot)}
sudo rm -f /var/lib/bitprotector/bitprotector.db.restore-pending
`)
    },
  }
}
