{{/* Returns the name of the TLS secret */}}
{{- define "1k-candidate-verifier.tls-secret-name" -}}
{{ .Release.Name }}-tls
{{- end }}

{{/* Returns the app name */}}
{{- define "1k-candidate-verifier.name" -}}
{{ .Release.Name }}
{{- end }}