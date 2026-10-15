import { type ChildProcess, spawn } from 'node:child_process'
import * as fs from 'node:fs/promises'
import * as path from 'node:path'
import * as vscode from 'vscode'

import type { Selection } from './selection.ts'

/**
 * The address file `tola dev` publishes for a site, and the shape it writes.
 *
 * The server owns both (`tola-build`'s `filesystem::dev_server`); the path is relative to the
 * site's own root, which is the directory holding the configuration.
 */
const ADDRESS_FILE = '.tola/dev-server.json'
const ADDRESS_VERSION = 1

/** How often the published address is read while the server starts. */
const ADDRESS_POLL_MS = 150

/** Silence this long means the server is not coming back, so the wait fails instead of hanging. */
const STARTUP_DEADLINE_MS = 300_000

export interface PublishedRoute {
  readonly output: string
  readonly route: string
}

export interface Publication {
  readonly revision: string
  readonly routes: PublishedRoute[]
}

export class PreviewServer {
  readonly ready: Promise<URL>
  readonly exited: Promise<void>
  private readonly child: ChildProcess
  private readonly cancellation = new AbortController()
  private readonly lifetime: vscode.Disposable
  private address: URL | undefined
  private stopped = false
  private closed = false
  private rejectStartup!: (error: Error) => void
  private startupTimer: NodeJS.Timeout | undefined
  private addressTimer: NodeJS.Timeout | undefined
  private killTimer: NodeJS.Timeout | undefined
  private publishing = 0

  constructor(
    selection: Selection,
    lifetime: vscode.CancellationToken,
    private readonly output: vscode.OutputChannel,
    private readonly changed: () => void,
  ) {
    const [subcommand, ...args] = selection.args
    if (subcommand !== 'lsp') {
      throw new Error(`Tola cannot derive the development command from \`${subcommand}\``)
    }
    if (lifetime.isCancellationRequested) throw new vscode.CancellationError()
    this.child = spawn(selection.command, ['dev', ...args, '--watch=true', '--interface=127.0.0.1'], {
      cwd: selection.folder.uri.fsPath,
      shell: false,
      stdio: ['ignore', 'pipe', 'pipe'],
    })
    output.appendLine(`Tola starts the development server for ${selection.configurationFile}`)
    const report = (chunk: Buffer) => {
      output.append(chunk.toString())
      changed()
    }
    this.child.stdout?.on('data', report)
    this.child.stderr?.on('data', report)
    const published = path.join(path.dirname(selection.config.fsPath), ADDRESS_FILE)
    const startedAt = Date.now()
    this.ready = new Promise<URL>((resolve, reject) => {
      this.rejectStartup = reject
      // A server that never publishes leaves nothing to open, so the author is told and the child
      // is ended here: the next Start Preview then starts a server rather than reusing this one.
      this.startupTimer = setTimeout(() => {
        reject(
          new Error(
            'Tola did not hear the development server publish its address; open Tola output, close any other `tola dev` for this site, and run Start Preview again',
          ),
        )
        this.stop()
      }, STARTUP_DEADLINE_MS)
      this.startupTimer.unref()
      const poll = async () => {
        if (this.stopped || this.address) return
        const address = await this.publishedAddress(published, startedAt)
        if (address) {
          clearTimeout(this.startupTimer)
          this.address = address
          resolve(address)
          return
        }
        this.addressTimer = setTimeout(() => void poll(), ADDRESS_POLL_MS)
        this.addressTimer.unref()
      }
      void poll()
      this.child.once(
        'error',
        (error) => {
          output.appendLine(`Development server startup failed: ${error.message}`)
          reject(
            new Error(
              'Tola could not start the development server. Open Tola output for details and run Start Preview again',
            ),
          )
        },
      )
    })
    this.exited = new Promise<void>((resolve) => {
      this.child.once('close', (code, signal) => {
        output.appendLine(`Development server exited (${signal ?? `exit ${code ?? 'unknown'}`})`)
        this.closed = true
        this.stopped = true
        this.cancellation.abort()
        this.lifetime.dispose()
        clearTimeout(this.startupTimer)
        this.rejectStartup(
          new Error(
            'Tola development server stopped before serving. Open Tola output for details and run Start Preview again',
          ),
        )
        changed()
        resolve()
      })
    })
    this.lifetime = lifetime.onCancellationRequested(() => this.stop())
  }

  get active(): boolean {
    return !this.stopped
  }

  get running(): boolean {
    return !this.stopped && this.address !== undefined
  }

  /**
   * The address this child published, while it is the one that wrote the file.
   *
   * A file that is absent, written before this server started, unreadable, or of another shape
   * names no address: the next poll reads it again rather than failing the start.
   */
  private async publishedAddress(published: string, startedAt: number): Promise<URL | undefined> {
    try {
      const written = await fs.stat(published)
      if (written.mtimeMs < startedAt - 1000) return undefined
      const state: unknown = JSON.parse(await fs.readFile(published, 'utf8'))
      if (typeof state !== 'object' || state === null || !('version' in state) || !('origin' in state)) {
        return undefined
      }
      const { version, origin } = state
      if (version !== ADDRESS_VERSION || typeof origin !== 'string') return undefined
      return new URL(origin)
    } catch {
      return undefined
    }
  }

  readPublication(source?: vscode.Uri, revision?: string): Promise<Publication> {
    return this.request('GET', source, revision)
  }

  get isPublishing(): boolean {
    return this.publishing > 0
  }

  async publish(source?: vscode.Uri): Promise<Publication> {
    this.publishing += 1
    this.changed()
    try {
      return await this.request('POST', source)
    } finally {
      this.publishing -= 1
      this.changed()
    }
  }

  private async request(
    method: 'GET' | 'POST',
    source?: vscode.Uri,
    revision?: string,
  ): Promise<Publication> {
    const address = await this.ready
    this.ensureActive()
    const url = new URL('/_tola/preview', address)
    if (source) url.searchParams.set('source', source.toString())
    if (revision) url.searchParams.set('revision', revision)
    let response: Response
    let publication: Publication & { error?: string }
    try {
      response = await fetch(url, {
        method,
        headers: { 'X-Tola-Preview': '1' },
        signal: this.cancellation.signal,
      })
      publication = await response.json() as Publication & { error?: string }
    } catch (error) {
      this.ensureActive()
      this.output.appendLine(`Development publication request failed: ${String(error)}`)
      throw new Error(
        'Tola could not read the development publication. Open Tola output or run Open Preview again',
      )
    }
    this.ensureActive()
    if (!response.ok) {
      throw new Error(publication.error ?? 'Tola could not publish the saved site; no new preview was opened')
    }
    return publication
  }

  ensureActive(): void {
    if (this.stopped) throw new vscode.CancellationError()
  }

  stop(): void {
    if (this.stopped) return
    this.stopped = true
    this.cancellation.abort()
    clearTimeout(this.addressTimer)
    clearTimeout(this.startupTimer)
    this.rejectStartup(new vscode.CancellationError())
    this.lifetime.dispose()
    if (this.closed) return
    // SIGINT lets Tola cancel its build and stop the generators it owns before exiting.
    this.child.kill('SIGINT')
    this.killTimer = setTimeout(() => {
      if (!this.closed) this.child.kill('SIGKILL')
    }, 5000)
    this.killTimer.unref()
  }
}
