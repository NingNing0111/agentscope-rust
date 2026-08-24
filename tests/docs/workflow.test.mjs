import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const workflowPath = resolve(
  dirname(fileURLToPath(import.meta.url)),
  '../../.github/workflows/docs.yml'
)

test('docs workflow keeps Pages metadata steps out of the read-only build job', async () => {
  const content = await readFile(workflowPath, 'utf8')
  // actions/configure-pages requires Pages API access that the build token
  // (contents: read) does not have and hard-fails when Pages is not enabled.
  assert.ok(
    !content.includes('configure-pages'),
    'configure-pages must not run in the read-only build job'
  )
  assert.ok(!content.includes('id: pages'), 'no Pages metadata step id may exist')
})

test('docs workflow grants Pages/OIDC permissions only to the deploy job', async () => {
  const content = await readFile(workflowPath, 'utf8')
  const buildSection = content.split(/^  build:/m)[1]?.split(/^  deploy:/m)[0] ?? content
  const deploySection = content.split(/^  deploy:/m)[1] ?? ''

  assert.ok(content.includes('contents: read'), 'top level must stay contents: read')
  assert.ok(!buildSection.includes('pages: write'), 'build job must not hold pages: write')
  assert.ok(!buildSection.includes('id-token: write'), 'build job must not hold id-token: write')
  assert.ok(deploySection.includes('pages: write'), 'deploy job must hold pages: write')
  assert.ok(deploySection.includes('id-token: write'), 'deploy job must hold id-token: write')
})

test('docs workflow deploys only on workflow_dispatch or push to master', async () => {
  const content = await readFile(workflowPath, 'utf8')
  assert.ok(content.includes("github.event_name == 'workflow_dispatch'"))
  assert.ok(content.includes("github.ref == 'refs/heads/master'"))
  assert.ok(content.includes('upload-pages-artifact'))
  assert.ok(content.includes('deploy-pages'))
})

test('docs workflow keeps Playwright opt-in and avoids apt dependency installation', async () => {
  const content = await readFile(workflowPath, 'utf8')
  const installStep = content.split('- name: Install Playwright Chromium')[1]?.split('\n\n')[0] ?? ''
  const e2eStep = content.split('- name: Browser and accessibility tests')[1]?.split('\n\n')[0] ?? ''

  assert.ok(content.includes('run_e2e:'), 'manual workflow must expose the run_e2e switch')
  assert.match(installStep, /if: \$\{\{ inputs\.run_e2e == true \}\}/)
  assert.match(e2eStep, /if: \$\{\{ inputs\.run_e2e == true \}\}/)
  assert.ok(!content.includes('run: npx playwright install --with-deps'), 'Playwright install must not invoke apt')
  assert.match(installStep, /npx playwright install chromium/)
})
