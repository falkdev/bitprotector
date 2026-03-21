export interface SystemStatus {
  files_tracked: number
  files_mirrored: number
  pending_sync: number
  integrity_issues: number
  drive_pairs: number
  degraded_pairs: number
  active_secondary_pairs: number
  rebuilding_pairs: number
  quiescing_pairs: number
}
