export type PlaybackStatus = "Playing" | "Paused" | "Stopped" | "Unknown";
export interface MediaTrack {
  title: string;
  artist: string | null;
  album: string | null;
  albumArtist: string | null;
  durationMs: number | null;
  trackNumber: number | null;
  genres: string[];
}
export interface MediaSession {
  id: string;
  source: { id: string; name: string | null };
  track: MediaTrack | null;
  playback: {
    status: PlaybackStatus;
    positionMs: number;
    playbackRate: number;
  };
}
export type Control =
  "play" | "pause" | "toggle" | "next" | "previous" | "seek";
export interface NowPlayingApi {
  current(): Promise<MediaSession | null>;
  sessions(): Promise<MediaSession[]>;
  control(
    command: Control,
    sessionId: string,
    positionMs?: number,
  ): Promise<void>;
}
export interface NativeBinding {
  current(): Promise<string | null>;
  sessions(): Promise<string>;
  control(
    command: string,
    sessionId: string,
    positionMs?: number,
  ): Promise<void>;
}
