/**
 * True for the Android/iOS WebViews the app ships with.
 *
 * The user agent is the only signal available to the frontend: the native
 * window is already fullscreen on mobile, so window-level affordances such as
 * the reader's fullscreen toggle must be hidden there. Battery HUD and
 * fullscreen use the same check so a narrow desktop window is never treated as
 * a phone.
 */
export function isMobileWebView(
  userAgent: string = navigator.userAgent,
): boolean {
  return /Android|iPhone|iPad|iPod/i.test(userAgent);
}
