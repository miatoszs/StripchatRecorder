/**
 * Recording File Type Definitions
 */

/*Post-processing module execution result */
export interface PpModuleResult {
	/*Module ID */
	module_id: string;
	/*Whether succeeded */
	success: boolean;
	/*Result message */
	message: string;
}

/**
 * Recording file status
 *
 * - `recording`       —
 * - `merging_waiting` — （）
 * - `merging`         —  TS
 * - `pp_waiting`      — （）
 * - `pp_running`      —
 * - `pp_error`        —
 * - `finish`          —
 */
export type RecordingStatus =
	| "recording"
	| "merging_waiting"
	| "merging"
	| "pp_waiting"
	| "pp_running"
	| "pp_error"
	| "finish";

/*Recording file metadata */
export interface RecordingFile {
	/** （）/ Filename (with extension) */
	name: string;
	/*Full file path */
	path: string;
	/** （）/ File size (bytes) */
	size_bytes: number;
	/** （ISO ）/ Recording start time (ISO string) */
	started_at: string;
	/*Whether currently recording */
	is_recording: boolean;
	/*Recorded duration (seconds), updated in real-time while recording */
	record_duration_secs: number | null;
	/*Actual video duration (seconds), obtained via ffprobe and stored in meta */
	video_duration_secs: number | null;
	/** （ meta ）/ Current processing status (from meta file) */
	status?: RecordingStatus | null;
	/** （ meta ）/ Per-module post-processing results (from meta file) */
	pp_results?: PpModuleResult[] | null;
	/** （ meta ）/ Module output paths (from meta file) */
	module_outputs?: Record<string, string> | null;
	/** （）/ Total successfully downloaded segments (updated in real-time while recording) */
	segments_downloaded?: number | null;
	/** （）/ Total failed segment downloads (updated in real-time while recording) */
	segments_failed?: number | null;
}
