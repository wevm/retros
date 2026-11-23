/**
 * The contact page's script: loads office hours from an endpoint that does not exist yet, which
 * logs an error to the console.
 *
 * @module
 */

const response = await fetch('/api/hours')
if (!response.ok) console.error(`Could not load office hours: ${response.status}`)
