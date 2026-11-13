import { useEffect, useState } from 'react';
import { Tv, Volume2, History, Check, ShieldAlert, Monitor, Sparkles, OctagonAlert, Keyboard, RotateCcw, ArrowDownCircle, RefreshCw, ExternalLink, Link2, Copy, LogOut } from 'lucide-react';
import type { AniListStatus, PlayerChoice, PlayersInfo, Settings } from './types';
import { ANILIST_AUTH_URL, QUALITIES } from './types';

type Props = {
  settings: Settings;
  players: PlayersInfo | null;
  historyCount: number;
  onSave: (next: Settings) => Promise<void>;
  onClearHistory: () => Promise<void>;
};

const PLAYER_OPTIONS: { id: PlayerChoice; label: string; desc: string }[] = [
  { id: 'mpv', label: 'mpv Player', desc: 'Recommended · Supports auto-advancing episodes' },
  { id: 'vlc', label: 'VLC Media Player', desc: 'Alternative external player' },
];

export function SettingsPage({ settings, players, historyCount, onSave, onClearHistory }: Props) {
  const [error, setError] = useState('');
  const [savedBadge, setSavedBadge] = useState(false);
  const [mpvPath, setMpvPath] = useState(settings.mpvPath);
  const [vlcPath, setVlcPath] = useState(settings.vlcPath);
  const [confirmClear, setConfirmClear] = useState(false);
  const [recordingPanicKey, setRecordingPanicKey] = useState(false);
  const [updateUrl, setUpdateUrl] = useState(settings.updateUrl || '');
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const [updateStatus, setUpdateStatus] = useState<string | null>(null);
  const [ani, setAni] = useState<AniListStatus | null>(null);
  const [aniToken, setAniToken] = useState('');
  const [aniBusy, setAniBusy] = useState(false);
  const [aniError, setAniError] = useState('');
  const [linkCopied, setLinkCopied] = useState(false);

  useEffect(() => {
    void refreshAniList();
  }, []);

  useEffect(() => {
    setMpvPath(settings.mpvPath);
    setVlcPath(settings.vlcPath);
    setUpdateUrl(settings.updateUrl || '');
  }, [settings.mpvPath, settings.vlcPath]);

  async function update(patch: Partial<Settings>) {
    setError('');
    try {
      await onSave({ ...settings, ...patch });
      setSavedBadge(true);
      setTimeout(() => setSavedBadge(false), 2000);
    } catch (e) {
      setError(typeof e === 'string' ? e : 'Could not save settings.');
    }
  }

  
  async function manualCheckUpdate() {
    setCheckingUpdate(true);
    setUpdateStatus(null);
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const res = await invoke<import('./types').UpdateCheckResult>('check_for_updates');
      if (res.hasUpdate) {
        setUpdateStatus(`Update available: v${res.latestVersion}`);
      } else {
        setUpdateStatus(`GravityAnime is up to date (v${res.currentVersion})`);
      }
    } catch (e) {
      setUpdateStatus(typeof e === 'string' ? e : 'Could not check for updates');
    } finally {
      setCheckingUpdate(false);
    }
  }

  async function aniInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(command, args);
  }

  async function refreshAniList() {
    try {
      setAni(await aniInvoke<AniListStatus>('anilist_status'));
    } catch {
      setAni(null); // not running inside Tauri (browser preview)
    }
  }

  async function aniLogin() {
    const token = aniToken.trim();
    if (!token) return;
    setAniBusy(true);
    setAniError('');
    try {
      setAni(await aniInvoke<AniListStatus>('anilist_login', { token }));
      setAniToken('');
    } catch (e) {
      setAniError(typeof e === 'string' ? e : 'Could not log in to AniList.');
    } finally {
      setAniBusy(false);
    }
  }

  async function aniLogout() {
    setAniBusy(true);
    setAniError('');
    try {
      setAni(await aniInvoke<AniListStatus>('anilist_logout'));
    } catch (e) {
      setAniError(typeof e === 'string' ? e : 'Could not log out of AniList.');
    } finally {
      setAniBusy(false);
    }
  }

  async function copyAuthLink() {
    try {
      await navigator.clipboard.writeText(ANILIST_AUTH_URL);
      setLinkCopied(true);
      setTimeout(() => setLinkCopied(false), 2000);
    } catch {
      /* clipboard unavailable: the link is still shown in the box */
    }
  }

  const found = (id: PlayerChoice) => (id === 'mpv' ? players?.mpv : players?.vlc) ?? null;
  const commitOnEnter = (e: React.KeyboardEvent<HTMLInputElement>) => e.key === 'Enter' && e.currentTarget.blur();

  return (
    <section className="page settings-dashboard">
      <div className="settings-header">
        <div>
          <h1 className="page-title">Settings</h1>
          <p className="page-sub">Configure playback behavior, external media players, and watch history.</p>
        </div>
        {savedBadge && (
          <div className="save-indicator">
            <Check size={14} /> Saved
          </div>
        )}
      </div>

      {error && <div className="notice error"><ShieldAlert size={16} /><span>{error}</span></div>}

      <div className="cards-grid">
        {/* Left Column / Card 1: Player Selection */}
        <div className="card-box">
          <div className="card-box-header">
            <div className="card-box-title-wrap">
              <Tv size={18} className="card-icon" />
              <div>
                <h2 className="card-box-title">Media Player</h2>
                <p className="card-box-sub">Choose which player GravityAnime uses for streaming.</p>
              </div>
            </div>
          </div>

          <div className="player-selector-list" role="radiogroup" aria-label="Video player">
            {PLAYER_OPTIONS.map(({ id, label, desc }) => {
              const path = found(id);
              const active = settings.player === id;
              return (
                <button
                  key={id}
                  type="button"
                  role="radio"
                  aria-checked={active}
                  className={`player-select-card ${active ? 'active' : ''}`}
                  onClick={() => update({ player: id })}
                >
                  <div className="player-card-icon">
                    <Monitor size={18} />
                  </div>
                  <div className="player-card-content">
                    <div className="player-card-head">
                      <strong>{label}</strong>
                      {active && <span className="active-pill">Selected</span>}
                    </div>
                    <span className="player-card-desc">{desc}</span>
                    <span className={`status-pill ${path ? 'ok' : 'missing'}`}>
                      {players ? (path ? `Detected: ${path}` : 'Not detected') : 'Checking…'}
                    </span>
                  </div>
                </button>
              );
            })}
          </div>

          {players && !players.selectedFound && (
            <div className="notice warn mt-3">
              <ShieldAlert size={16} />
              <span>{players.message}</span>
            </div>
          )}

          <div className="card-form-section">
            <label className="card-field-label" htmlFor="mpv-path">
              mpv executable path <span className="label-sub">(optional)</span>
            </label>
            <input
              id="mpv-path"
              className="card-input"
              value={mpvPath}
              placeholder="e.g. C:\Tools\mpv\mpv.exe"
              onChange={e => setMpvPath(e.target.value)}
              onBlur={() => mpvPath !== settings.mpvPath && update({ mpvPath })}
              onKeyDown={commitOnEnter}
            />

            <label className="card-field-label mt-3" htmlFor="vlc-path">
              VLC executable path <span className="label-sub">(optional)</span>
            </label>
            <input
              id="vlc-path"
              className="card-input"
              value={vlcPath}
              placeholder="e.g. C:\Program Files\VideoLAN\VLC\vlc.exe"
              onChange={e => setVlcPath(e.target.value)}
              onBlur={() => vlcPath !== settings.vlcPath && update({ vlcPath })}
              onKeyDown={commitOnEnter}
            />
          </div>
        </div>

        {/* Right Column / Card 2: Playback Settings */}
        <div className="card-box">
          <div className="card-box-header">
            <div className="card-box-title-wrap">
              <Volume2 size={18} className="card-icon" />
              <div>
                <h2 className="card-box-title">Playback Preferences</h2>
                <p className="card-box-sub">Configure default streaming qualities and audio.</p>
              </div>
            </div>
          </div>

          <div className="card-rows">
            <div className="card-row">
              <div className="row-text">
                <strong>Default Audio Language</strong>
                <small>Choose between Subtitled or Dubbed audio by default.</small>
              </div>
              <div className="segmented-cards" role="group" aria-label="Default audio">
                <button
                  type="button"
                  className={!settings.defaultDub ? 'active' : ''}
                  aria-pressed={!settings.defaultDub}
                  onClick={() => update({ defaultDub: false })}
                >
                  Sub
                </button>
                <button
                  type="button"
                  className={settings.defaultDub ? 'active' : ''}
                  aria-pressed={settings.defaultDub}
                  onClick={() => update({ defaultDub: true })}
                >
                  Dub
                </button>
              </div>
            </div>

            <div className="card-row">
              <div className="row-text">
                <strong>Streaming Quality</strong>
                <small>Preferred stream resolution when available.</small>
              </div>
              <div className="select-container">
                <select
                  className="card-select"
                  value={settings.defaultQuality}
                  onChange={e => update({ defaultQuality: e.target.value })}
                  aria-label="Default quality"
                >
                  {QUALITIES.map(q => (
                    <option key={q} value={q}>{q}</option>
                  ))}
                </select>
              </div>
            </div>

            <div className="card-row">
              <div className="row-text">
                <strong>Continuous Playback</strong>
                <small>Automatically start next episode (mpv only).</small>
              </div>
              <label className="switch">
                <input
                  type="checkbox"
                  checked={settings.autoPlayNext}
                  onChange={e => update({ autoPlayNext: e.target.checked })}
                />
                <span className="switch-track" />
                <span className="visually-hidden">Play next episode automatically</span>
              </label>
            </div>
          </div>
        </div>

        
        {/* Card: Emergency Panic Key */}
        <div className="card-box panic-card">
          <div className="card-box-header">
            <div className="card-box-title-wrap">
              <OctagonAlert size={18} className="card-icon text-red" />
              <div>
                <h2 className="card-box-title">Emergency Panic Key</h2>
                <p className="card-box-sub">Immediately kills active media player processes and exits AniGUI.</p>
              </div>
            </div>
          </div>

          <div className="card-content-block">
            <div className="panic-key-row">
              <div className="row-text">
                <strong>Assigned Trigger Key</strong>
                <small>Press this key anywhere while GravityAnime is focused to instantly terminate everything.</small>
              </div>

              <div className="panic-key-actions">
                <button
                  type="button"
                  className={`panic-key-btn ${recordingPanicKey ? 'recording' : ''}`}
                  onClick={() => setRecordingPanicKey(true)}
                  onKeyDown={(e) => {
                    if (!recordingPanicKey) return;
                    e.preventDefault();
                    e.stopPropagation();
                    const keyName = e.key === ' ' ? 'Space' : e.key;
                    update({ panicKey: keyName.toUpperCase() });
                    setRecordingPanicKey(false);
                  }}
                  onBlur={() => setRecordingPanicKey(false)}
                  title="Click to change shortcut key"
                >
                  <Keyboard size={15} />
                  <span>{recordingPanicKey ? 'Press key…' : (settings.panicKey || 'F12')}</span>
                </button>

                {settings.panicKey !== 'F12' && (
                  <button
                    type="button"
                    className="card-btn secondary reset-btn"
                    title="Reset to default (F12)"
                    onClick={() => update({ panicKey: 'F12' })}
                  >
                    <RotateCcw size={14} /> Reset
                  </button>
                )}
              </div>
            </div>

            <p className="panic-key-tip">
              Default is <strong>F12</strong>. Supports function keys (F1–F12), Escape, or any custom key.
            </p>
          </div>
        </div>

        {/* Card 3: Watch History */}
        <div className="card-box">
          <div className="card-box-header">
            <div className="card-box-title-wrap">
              <History size={18} className="card-icon" />
              <div>
                <h2 className="card-box-title">Watch History</h2>
                <p className="card-box-sub">Local playback and resume progress data.</p>
              </div>
            </div>
          </div>

          <div className="card-content-block">
            <div className="history-stat-badge">
              <span className="stat-number">{historyCount}</span>
              <span className="stat-label">{historyCount === 1 ? 'Anime in history' : 'Anime in history'}</span>
            </div>
            <p className="history-note">
              History is stored locally on this machine and enables the "Continue Watching" row.
            </p>

            <div className="card-action-bar">
              {confirmClear ? (
                <div className="btn-group">
                  <button
                    type="button"
                    className="card-btn danger"
                    onClick={async () => {
                      await onClearHistory();
                      setConfirmClear(false);
                    }}
                  >
                    Confirm Clear
                  </button>
                  <button
                    type="button"
                    className="card-btn secondary"
                    onClick={() => setConfirmClear(false)}
                  >
                    Cancel
                  </button>
                </div>
              ) : (
                <button
                  type="button"
                  className="card-btn secondary"
                  disabled={historyCount === 0}
                  onClick={() => setConfirmClear(true)}
                >
                  Clear watch history
                </button>
              )}
            </div>
          </div>
        </div>

        
        {/* Card: AniList */}
        <div className="card-box">
          <div className="card-box-header">
            <div className="card-box-title-wrap">
              <Link2 size={18} className="card-icon" />
              <div>
                <h2 className="card-box-title">AniList</h2>
                <p className="card-box-sub">Update your AniList progress when an episode finishes.</p>
              </div>
            </div>
          </div>

          {ani?.loggedIn ? (
            <div className="card-rows">
              <div className="card-row">
                <div className="row-text">
                  <strong>{ani.username ? `Logged in as ${ani.username}` : 'Logged in'}</strong>
                  <small>{ani.username ? 'Finished episodes are added to your list.' : ani.message}</small>
                </div>
                <button type="button" className="card-btn secondary" disabled={aniBusy} onClick={aniLogout}>
                  <LogOut size={14} /> Log out
                </button>
              </div>

              <div className="card-row">
                <div className="row-text">
                  <strong>Sync Progress</strong>
                  <small>Only raises your progress, never lowers it.</small>
                </div>
                <label className="switch">
                  <input
                    type="checkbox"
                    checked={settings.syncAnilist}
                    onChange={e => update({ syncAnilist: e.target.checked })}
                  />
                  <span className="switch-track" />
                  <span className="visually-hidden">Sync watch progress to AniList</span>
                </label>
              </div>
            </div>
          ) : (
            <div className="card-content-block">
              {ani && ani.message && <p className="update-help-note">{ani.message}</p>}

              <div className="card-form-section">
                <label className="card-field-label" htmlFor="anilist-link">
                  1. Open this link in your browser and authorize the app
                </label>
                {ANILIST_AUTH_URL ? (
                  <div className="update-status-row">
                    <input id="anilist-link" className="card-input" readOnly value={ANILIST_AUTH_URL} onFocus={e => e.currentTarget.select()} />
                    <button type="button" className="card-btn secondary" onClick={copyAuthLink}>
                      <Copy size={14} /> {linkCopied ? 'Copied' : 'Copy'}
                    </button>
                  </div>
                ) : (
                  <p className="update-help-note">
                    Set <code>ANILIST_CLIENT_ID</code> in <code>types.ts</code> first (create a client at anilist.co/settings/developer
                    with redirect URL <code>https://anilist.co/api/v2/oauth/pin</code>).
                  </p>
                )}
              </div>

              <div className="card-form-section mt-3">
                <label className="card-field-label" htmlFor="anilist-token">
                  2. Paste the token AniList shows you
                </label>
                <input
                  id="anilist-token"
                  className="card-input"
                  type="password"
                  autoComplete="off"
                  value={aniToken}
                  placeholder="Access token"
                  onChange={e => setAniToken(e.target.value)}
                  onKeyDown={e => e.key === 'Enter' && void aniLogin()}
                />
              </div>

              <div className="update-status-row mt-3">
                <button type="button" className="card-btn secondary" disabled={aniBusy || !aniToken.trim()} onClick={aniLogin}>
                  {aniBusy ? 'Checking…' : 'Log in'}
                </button>
                {aniError && <span className="update-status-text">{aniError}</span>}
              </div>
            </div>
          )}
        </div>

        {/* Card: Auto Updates & GitHub Release Channel */}
        <div className="card-box">
          <div className="card-box-header">
            <div className="card-box-title-wrap">
              <ArrowDownCircle size={18} className="card-icon" />
              <div>
                <h2 className="card-box-title">Updates & Releases</h2>
                <p className="card-box-sub">Auto-detect new versions from GitHub release metadata.</p>
              </div>
            </div>
          </div>

          <div className="card-content-block">
            <div className="card-form-section">
              <label className="card-field-label" htmlFor="update-url">
                Update Manifest URL (raw update-info.json)
              </label>
              <input
                id="update-url"
                className="card-input"
                value={updateUrl}
                placeholder="https://raw.githubusercontent.com/.../update-info.json"
                onChange={e => setUpdateUrl(e.target.value)}
                onBlur={() => updateUrl !== settings.updateUrl && update({ updateUrl })}
                onKeyDown={commitOnEnter}
              />
            </div>

            <div className="update-status-row mt-3">
              <button
                type="button"
                className="card-btn secondary"
                disabled={checkingUpdate}
                onClick={manualCheckUpdate}
              >
                <RefreshCw size={14} className={checkingUpdate ? 'spinner' : ''} />
                {checkingUpdate ? 'Checking…' : 'Check for Updates'}
              </button>

              {updateStatus && (
                <span className="update-status-text">{updateStatus}</span>
              )}
            </div>

            <p className="update-help-note">
              To publish a new version, simply update <code>update-info.json</code> in your repository and push to GitHub.
            </p>
          </div>
        </div>

        {/* Card 4: About / Environment Information */}
        <div className="card-box">
          <div className="card-box-header">
            <div className="card-box-title-wrap">
              <Sparkles size={18} className="card-icon" />
              <div>
                <h2 className="card-box-title">Application Info</h2>
                <p className="card-box-sub">GravityAnime client and backend version details.</p>
              </div>
            </div>
          </div>

          <div className="info-list">
            <div className="info-item">
              <span className="info-label">Version</span>
              <span className="info-val">v0.1.0</span>
            </div>
            <div className="info-item">
              <span className="info-label">Framework</span>
              <span className="info-val">Tauri 2.0 + React</span>
            </div>
            <div className="info-item">
              <span className="info-label">Playback Provider</span>
              <span className="info-val">{settings.player.toUpperCase()}</span>
            </div>
            <div className="info-item">
              <span className="info-label">Status</span>
              <span className="info-badge-success">Operational</span>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}