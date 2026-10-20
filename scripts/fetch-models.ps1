# Downloads the face models into ./models and verifies their checksums.
# YuNet (MIT) and SFace (Apache-2.0) from the OpenCV model zoo.
$ErrorActionPreference = 'Stop'
$dir = Join-Path $PSScriptRoot '..\models'
New-Item -ItemType Directory -Force $dir | Out-Null
$models = @(
  @{ name = 'face_detection_yunet_2023mar.onnx'
     url  = 'https://github.com/opencv/opencv_zoo/raw/main/models/face_detection_yunet/face_detection_yunet_2023mar.onnx'
     sha  = '8f2383e4dd3cfbb4553ea8718107fc0423210dc964f9f4280604804ed2552fa4' },
  @{ name = 'face_recognition_sface_2021dec.onnx'
     url  = 'https://github.com/opencv/opencv_zoo/raw/main/models/face_recognition_sface/face_recognition_sface_2021dec.onnx'
     sha  = '0ba9fbfa01b5270c96627c4ef784da859931e02f04419c829e83484087c34e79' }
)
foreach ($m in $models) {
  $path = Join-Path $dir $m.name
  if (-not (Test-Path $path)) {
    Write-Host "Downloading $($m.name)"
    Invoke-WebRequest $m.url -OutFile $path
  }
  $hash = (Get-FileHash $path -Algorithm SHA256).Hash.ToLower()
  if ($hash -ne $m.sha) { Remove-Item $path; throw "Checksum mismatch for $($m.name)" }
}
Write-Host 'Models ready.'
