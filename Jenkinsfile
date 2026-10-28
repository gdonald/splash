// Runs in the home lab's Jenkins, in one pod running the splash CI image built
// from docker/. When every stage passes, Jenkins reports the
// continuous-integration/jenkins/branch status, which starts
// .github/workflows/ci.yml on the same commit.

// tarpaulin traces the test binaries with ptrace and turns off address space
// randomization for them, which the container runtime's default seccomp
// profile blocks.
def buildPod = '''
  apiVersion: v1
  kind: Pod
  spec:
    containers:
      - name: ci
        image: harbor.home.gregdonald.com/splash/ci:latest
        imagePullPolicy: Always
        command: ["sleep"]
        args: ["infinity"]
        securityContext:
          seccompProfile:
            type: Unconfined
        resources:
          requests:
            cpu: "4"
            memory: 8Gi
          limits:
            cpu: "8"
            memory: 8Gi
'''

pipeline {
  agent {
    kubernetes {
      defaultContainer 'ci'
      yaml buildPod
    }
  }

  options {
    disableConcurrentBuilds(abortPrevious: true)
    timeout(time: 120, unit: 'MINUTES')
  }

  // A failing stage marks the build failed and the stages after it still
  // run, so one run reports every failure.
  stages {
    stage('lint') {
      steps {
        catchError(buildResult: 'FAILURE', stageResult: 'FAILURE') {
          sh 'cargo fmt -- --check'
          sh 'cargo clippy -- -D warnings'
        }
      }
    }

    stage('test') {
      steps {
        catchError(buildResult: 'FAILURE', stageResult: 'FAILURE') {
          sh 'cargo build'
          sh 'cargo test'
        }
      }
    }

    stage('coverage') {
      steps {
        catchError(buildResult: 'FAILURE', stageResult: 'FAILURE') {
          sh 'cargo tarpaulin --all-features --workspace --timeout 120 --out xml'
        }
      }
      post {
        always {
          archiveArtifacts artifacts: 'cobertura.xml', allowEmptyArchive: true
        }
      }
    }
  }
}
