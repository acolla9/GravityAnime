export type Status = { appName: string; version: string; engineConnected: boolean; platform: string; message: string };
export type AnimeResult = { id: string; title: string; poster?: string | null };
export type SearchResponse = { query: string; results: AnimeResult[]; engineConnected: boolean; message: string };
export type Episode = { id: string; number: string };
export type EpisodesResponse = { animeId: string; episodes: Episode[]; message: string };

export type PlayerChoice = 'mpv' | 'vlc';
export type Settings = {
  player: PlayerChoice;
  mpvPath: string;
  vlcPath: string;
  defaultDub: boolean;
  defaultQuality: string;
  autoPlayNext: boolean;
  panicKey: string;
  updateUrl: string;
  syncAnilist: boolean;
};
export type PlayersInfo = { selected: PlayerChoice; mpv: string | null; vlc: string | null; selectedFound: boolean; message: string };

export type UpdateCheckResult = {
  hasUpdate: boolean;
  currentVersion: string;
  latestVersion: string;
  releaseDate?: string;
  notes?: string;
  downloadUrl?: string;
  allPreviousVersions: { version: string; releaseDate?: string; notes?: string; downloadUrl?: string }[];
};

export type AniListStatus = { loggedIn: boolean; username?: string | null; message: string };

export type HistoryEntry = {
  animeId: string;
  animeTitle: string;
  poster?: string | null;
  episodeId: string;
  episodeNumber: string;
  completed: boolean;
  updatedAt: number;
};

export type PlaybackStarted = { episodeId: string; player: string; quality: string; hasSubtitles: boolean; hasSeparateAudio: boolean; message: string };
export type PlaybackEnded = { animeId: string; episodeId: string; exitCode: number | null; failed: boolean; completed: boolean; message: string };

export type NowPlaying = {
  anime: AnimeResult;
  episode: Episode;
  /** Snapshot of the episode list, so Next/Previous work even after you search for something else. */
  episodes: Episode[];
  state: 'starting' | 'playing' | 'finished' | 'closed';
};

export const QUALITIES = ['best', '1080p', '720p', '480p', '360p', 'worst'];

export const DEFAULT_UPDATE_URL = 'https://raw.githubusercontent.com/AidenColla/AniGUI/main/update-info.json';

export const DEFAULT_SETTINGS: Settings = {
  player: 'mpv',
  mpvPath: '',
  vlcPath: '',
  defaultDub: false,
  defaultQuality: 'best',
  autoPlayNext: true,
  panicKey: 'F12',
  updateUrl: DEFAULT_UPDATE_URL,
  syncAnilist: true,
};

/**
 * Client ID of your AniList API client (https://anilist.co/settings/developer).
 * Set its redirect URL to https://anilist.co/api/v2/oauth/pin so AniList shows the token to copy.
 */
export const ANILIST_CLIENT_ID = '56565';

export const ANILIST_AUTH_URL = ANILIST_CLIENT_ID
  ? `https://anilist.co/api/v2/oauth/authorize?client_id=${ANILIST_CLIENT_ID}&response_type=token`
  : '';