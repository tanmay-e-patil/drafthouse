export default defineEventHandler((event) => {
  const path = event.path

  if (path.startsWith('/assets/')) {
    setResponseHeader(event, 'Cache-Control', 'public, max-age=31536000, immutable')
  } else {
    setResponseHeader(event, 'Cache-Control', 'public, max-age=0, must-revalidate')
  }

  setResponseHeader(event, 'X-Content-Type-Options', 'nosniff')
  setResponseHeader(event, 'X-Frame-Options', 'DENY')
  setResponseHeader(event, 'X-XSS-Protection', '0')
  setResponseHeader(event, 'Referrer-Policy', 'strict-origin-when-cross-origin')
  setResponseHeader(event, 'Cross-Origin-Opener-Policy', 'same-origin')
  setResponseHeader(event, 'Cross-Origin-Resource-Policy', 'same-origin')
  setResponseHeader(event, 'Permissions-Policy', 'camera=(), microphone=(), geolocation=()')

  if (process.env.NODE_ENV === 'production') {
    setResponseHeader(
      event,
      'Strict-Transport-Security',
      'max-age=63072000; includeSubDomains; preload'
    )
  }

  const origin = getHeader(event, 'origin')
  const appOrigin = process.env.APP_ORIGIN
  if (origin && appOrigin) {
    setResponseHeader(event, 'Access-Control-Allow-Origin', appOrigin)
    setResponseHeader(event, 'Access-Control-Allow-Credentials', 'true')
  }
})
