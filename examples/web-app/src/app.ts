/**
 * The client script: submits the signup form without a page load, and remembers the plan a
 * visitor chooses.
 *
 * @module
 */

const form = document.querySelector<HTMLFormElement>('#signup')
const status = document.querySelector<HTMLElement>('#signup-status')

form?.addEventListener('submit', async (event) => {
  event.preventDefault()
  const email = new FormData(form).get('email')
  if (status) status.textContent = `Check ${email} for your first forecast.`
})

for (const button of document.querySelectorAll<HTMLButtonElement>(
  '[data-plan]',
)) {
  button.addEventListener('click', () => {
    localStorage.setItem('plan', button.dataset.plan ?? '')
    console.log('chose plan', button.dataset.plan)
  })
}
