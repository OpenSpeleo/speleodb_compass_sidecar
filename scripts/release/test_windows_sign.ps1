# Exercise the real callback without Azure, PowerShell Gallery, or signing credentials.
$ErrorActionPreference = 'Stop'
$signer = Join-Path $PSScriptRoot 'windows-sign.ps1'
$fixtureDirectory = Join-Path ([IO.Path]::GetTempPath()) ('sidecar-signing-test-' + [Guid]::NewGuid())
$testState = @{}
$testState.FixturePath = Join-Path $fixtureDirectory 'SpeleoDB Compass Sidecar.exe'
$testState.Assertion = 'fixture-oidc-assertion-must-not-appear-in-output'
$testState.RequestToken = 'fixture-request-token-must-not-appear-in-output'
$testState.TestCount = 0
$fixtureEnvironment = @{
    AZURE_SIGNING_ENDPOINT = 'https://signing.invalid'
    AZURE_SIGNING_ACCOUNT = 'fixture-account'
    AZURE_SIGNING_PROFILE = 'fixture-profile'
    ARTIFACT_SIGNING_MODULE_VERSION = 'fixture-version'
    AZURE_CLIENT_ID = 'fixture-client'
    AZURE_TENANT_ID = 'fixture-tenant'
    AZURE_SUBSCRIPTION_ID = 'fixture-subscription'
    ACTIONS_ID_TOKEN_REQUEST_URL = 'https://oidc.invalid/token?existing=1'
    ACTIONS_ID_TOKEN_REQUEST_TOKEN = $testState.RequestToken
}
$savedEnvironment = @{}
foreach ($name in $fixtureEnvironment.Keys) {
    $savedEnvironment[$name] = [Environment]::GetEnvironmentVariable($name)
}
$savedExitCode = $global:LASTEXITCODE

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

# Function mocks shadow the external commands only within this test script.
function Import-Module {
    param($Name, $RequiredVersion, $ErrorAction)
    $testState.Events.Add('import')
    Assert-True ($Name -eq 'ArtifactSigning') 'Unexpected module import'
    Assert-True ($RequiredVersion -eq $env:ARTIFACT_SIGNING_MODULE_VERSION) 'Module version was not pinned'
    Assert-True ($ErrorAction -eq 'Stop') 'Module import must fail on errors'
}

function az {
    $command = $args -join ' '
    if ($command.StartsWith('account get-access-token ')) {
        $testState.TokenAttempts++
        $testState.Events.Add("token-$($testState.TokenAttempts)")
        Assert-True ($command.Contains('--resource https://codesigning.azure.net')) 'Wrong token audience'
        Assert-True ($command.Contains('--output none')) 'Token output must be suppressed'
        $global:LASTEXITCODE = if ($testState.Renew -and ($testState.TokenAttempts -eq 1 -or $testState.Scenario -eq 'token-error')) { 1 } else { 0 }
    } elseif ($command.StartsWith('login ')) {
        $testState.Events.Add('login')
        Assert-True ($command.Contains("--federated-token $($testState.Assertion)")) 'Login did not use the fresh assertion'
        Assert-True ($command.Contains("--username $env:AZURE_CLIENT_ID")) 'Wrong signing identity'
        Assert-True ($command.Contains("--tenant $env:AZURE_TENANT_ID")) 'Wrong signing tenant'
        # Simulate a CLI that leaks its assertion on multiple output streams.
        # The callback must suppress them on both success and failure.
        Write-Output $testState.Assertion
        Write-Warning $testState.Assertion
        if ($testState.Scenario -eq 'login-throw') { throw $testState.Assertion }
        $global:LASTEXITCODE = if ($testState.Scenario -eq 'login-error') { 1 } else { 0 }
    } elseif ($command.StartsWith('account set ')) {
        $testState.Events.Add('subscription')
        Assert-True ($command.Contains("--subscription $env:AZURE_SUBSCRIPTION_ID")) 'Wrong signing subscription'
        $global:LASTEXITCODE = if ($testState.Scenario -eq 'subscription-error') { 1 } else { 0 }
    } else {
        throw 'Unexpected Azure CLI invocation'
    }
}

function Invoke-RestMethod {
    param($Method, $TimeoutSec, $Uri, $Headers)
    $testState.Events.Add('oidc')
    $testState.RequestUri = $Uri
    Assert-True ($Method -eq 'Get' -and $TimeoutSec -eq 30) 'OIDC request must be bounded'
    Assert-True ($Headers.Authorization -eq "bearer $($testState.RequestToken)") 'OIDC request authentication missing'
    if ($testState.Scenario -eq 'oidc-error') { throw $testState.RequestToken }
    return @{ value = $(if ($testState.Scenario -eq 'empty-assertion') { '' } else { $testState.Assertion }) }
}

function Invoke-ArtifactSigning {
    param(
        $Endpoint, $CodeSigningAccountName, $CertificateProfileName, $Files,
        $FileDigest, $TimestampRfc3161, $TimestampDigest, $Description,
        [switch]$ExcludeEnvironmentCredential,
        [switch]$ExcludeWorkloadIdentityCredential,
        [switch]$ExcludeManagedIdentityCredential,
        [switch]$ExcludeSharedTokenCacheCredential,
        [switch]$ExcludeVisualStudioCredential,
        [switch]$ExcludeVisualStudioCodeCredential,
        [switch]$ExcludeAzurePowerShellCredential,
        [switch]$ExcludeAzureDeveloperCliCredential,
        [switch]$ExcludeInteractiveBrowserCredential
    )
    $testState.Events.Add('sign')
    $testState.SignArguments = @{} + $PSBoundParameters
    if ($testState.Scenario -eq 'sign-error') { throw 'Fixture signing failed' }
}

function Get-AuthenticodeSignature {
    param($LiteralPath)
    $testState.Events.Add('verify')
    Assert-True ($LiteralPath -eq (Resolve-Path -LiteralPath $testState.FixturePath).Path) 'Verified the wrong file'
    return [PSCustomObject]@{
        Status = $(if ($testState.Scenario -eq 'invalid-signature') { 'HashMismatch' } else { 'Valid' })
        TimeStamperCertificate = $(if ($testState.Scenario -eq 'no-timestamp') { $null } else { 'fixture-certificate' })
    }
}

function Invoke-Signer([string]$InputPath = $testState.FixturePath) {
    $failure = $null
    $captured = @(
        try { & $signer $InputPath *>&1 }
        catch { $failure = $_.Exception.Message; $_ }
    )
    $rendered = $captured | Out-String
    Assert-True (-not $rendered.Contains($testState.Assertion)) 'Callback exposed an OIDC assertion'
    Assert-True (-not $rendered.Contains($testState.RequestToken)) 'Callback exposed an OIDC request token'
    return [PSCustomObject]@{ Failure = $failure; Output = $rendered }
}

function Assert-Events([string]$Expected) {
    $actual = $testState.Events -join ','
    Assert-True ($actual -eq $Expected) "Expected callback order '$Expected', got '$actual'"
}

function Test-Case([string]$Name, [scriptblock]$Body) {
    foreach ($settingName in $fixtureEnvironment.Keys) {
        [Environment]::SetEnvironmentVariable($settingName, $fixtureEnvironment[$settingName])
    }
    $testState.Events = [Collections.Generic.List[string]]::new()
    $testState.TokenAttempts = 0
    $testState.Renew = $false
    $testState.Scenario = ''
    $testState.RequestUri = $null
    $testState.SignArguments = @{}
    & $Body
    $testState.TestCount++
    Write-Host "PASS: $Name"
}

try {
    [IO.Directory]::CreateDirectory($fixtureDirectory) | Out-Null
    [IO.File]::WriteAllText($testState.FixturePath, 'unsigned fixture; no executable content')

    Test-Case 'Reuse token, preserve spaces, and timestamp with Azure CLI credentials' {
        $result = Invoke-Signer
        Assert-True ($null -eq $result.Failure) "Unexpected signing failure: $($result.Failure)"
        Assert-Events 'import,token-1,sign,verify'
        $arguments = $testState.SignArguments
        Assert-True ($arguments.Files -eq (Resolve-Path -LiteralPath $testState.FixturePath).Path) 'Signing changed the file path'
        Assert-True ($arguments.Endpoint -eq $env:AZURE_SIGNING_ENDPOINT) 'Wrong signing endpoint'
        Assert-True ($arguments.CodeSigningAccountName -eq $env:AZURE_SIGNING_ACCOUNT) 'Wrong signing account'
        Assert-True ($arguments.CertificateProfileName -eq $env:AZURE_SIGNING_PROFILE) 'Wrong certificate profile'
        Assert-True ($arguments.FileDigest -eq 'SHA256' -and $arguments.TimestampDigest -eq 'SHA256') 'Wrong digest'
        Assert-True ($arguments.TimestampRfc3161 -eq 'http://timestamp.acs.microsoft.com') 'Wrong timestamp service'
        Assert-True ($arguments.Description -eq 'SpeleoDB Compass Sidecar') 'Wrong signing description'
        foreach ($credential in @('Environment', 'WorkloadIdentity', 'ManagedIdentity', 'SharedTokenCache', 'VisualStudio', 'VisualStudioCode', 'AzurePowerShell', 'AzureDeveloperCli', 'InteractiveBrowser')) {
            Assert-True ([bool]$arguments["Exclude${credential}Credential"]) "Unexpected credential fallback: $credential"
        }
    }

    foreach ($requestUrl in @('https://oidc.invalid/token', 'https://oidc.invalid/token?existing=1')) {
        Test-Case 'Renew expired session before signing' {
            $testState.Renew = $true
            $env:ACTIONS_ID_TOKEN_REQUEST_URL = $requestUrl
            $result = Invoke-Signer
            Assert-True ($null -eq $result.Failure) "Unexpected renewal failure: $($result.Failure)"
            Assert-Events 'import,token-1,oidc,login,subscription,token-2,sign,verify'
            $separator = if ($requestUrl.Contains('?')) { '&' } else { '?' }
            $expected = $requestUrl + $separator + 'audience=' + [Uri]::EscapeDataString('api://AzureADTokenExchange')
            Assert-True ($testState.RequestUri -eq $expected) 'OIDC URL did not preserve its existing query'
        }
    }

    foreach ($missing in @('AZURE_SIGNING_ENDPOINT', 'AZURE_SIGNING_ACCOUNT', 'AZURE_SIGNING_PROFILE', 'ARTIFACT_SIGNING_MODULE_VERSION')) {
        Test-Case "Reject missing $missing" {
            [Environment]::SetEnvironmentVariable($missing, '')
            $result = Invoke-Signer
            Assert-True ($result.Failure -eq "Missing $missing") 'Expected missing signing configuration error'
            Assert-Events ''
        }
    }
    Test-Case 'Reject absent signing input before loading tools' {
        $result = Invoke-Signer (Join-Path $fixtureDirectory 'missing.exe')
        Assert-True ($result.Failure.StartsWith('Signing input does not exist:')) 'Expected missing input error'
        Assert-Events ''
    }
    foreach ($missing in @('AZURE_CLIENT_ID', 'AZURE_TENANT_ID', 'AZURE_SUBSCRIPTION_ID', 'ACTIONS_ID_TOKEN_REQUEST_URL', 'ACTIONS_ID_TOKEN_REQUEST_TOKEN')) {
        Test-Case "Reject missing renewal setting $missing" {
            $testState.Renew = $true
            [Environment]::SetEnvironmentVariable($missing, '')
            $result = Invoke-Signer
            Assert-True ($result.Failure -eq "Missing $missing for Azure signing session renewal") 'Expected missing renewal configuration error'
            Assert-Events 'import,token-1'
        }
    }

    $failures = @(
        @('oidc-error', 'Unable to request a fresh GitHub OIDC assertion for Windows signing', 'import,token-1,oidc'),
        @('empty-assertion', 'GitHub returned an empty OIDC assertion for Windows signing', 'import,token-1,oidc'),
        @('login-error', 'Azure signing session renewal failed', 'import,token-1,oidc,login'),
        @('login-throw', 'Azure signing session renewal failed', 'import,token-1,oidc,login'),
        @('subscription-error', 'Unable to select the Azure signing subscription', 'import,token-1,oidc,login,subscription'),
        @('token-error', 'Unable to acquire an Azure Artifact Signing access token', 'import,token-1,oidc,login,subscription,token-2')
    )
    foreach ($case in $failures) {
        Test-Case "Stop after $($case[0]) without exposing assertions" {
            $testState.Renew = $true
            $testState.Scenario = $case[0]
            $result = Invoke-Signer
            Assert-True ($result.Failure -eq $case[1]) "Wrong authentication failure: $($result.Failure)"
            Assert-Events $case[2]
        }
    }
    Test-Case 'Stop after signing failure' {
        $testState.Scenario = 'sign-error'
        $result = Invoke-Signer
        Assert-True ($result.Failure -eq 'Fixture signing failed') 'Signing failure was ignored'
        Assert-Events 'import,token-1,sign'
    }
    foreach ($scenario in @('invalid-signature', 'no-timestamp')) {
        Test-Case "Reject $scenario" {
            $testState.Scenario = $scenario
            $result = Invoke-Signer
            Assert-True ($result.Failure.StartsWith('Expected a valid timestamped Authenticode signature:')) 'Unsigned or untimestamped output was accepted'
            Assert-Events 'import,token-1,sign,verify'
        }
    }
    Write-Host "Passed $($testState.TestCount) Windows signing callback tests."
} finally {
    foreach ($name in $savedEnvironment.Keys) {
        [Environment]::SetEnvironmentVariable($name, $savedEnvironment[$name])
    }
    $global:LASTEXITCODE = $savedExitCode
    if (Test-Path -LiteralPath $fixtureDirectory) {
        Remove-Item -LiteralPath $fixtureDirectory -Recurse -Force
    }
}
