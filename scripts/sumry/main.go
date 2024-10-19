package main

import (
	"bufio"
	"bytes"
	"flag"
	"fmt"
	"html/template"
	"io"
	"os"
	"regexp"

	"github.com/charmbracelet/log"
	"github.com/go-git/go-git/v5"
	"github.com/go-git/go-git/v5/plumbing"
	"github.com/go-git/go-git/v5/plumbing/object"
)

// this file is disgusting, I'm just prototyping :)

const (
	COMMIT_MAGIC_STRING = `^\[commit\]: # '(.+)'$`
	SUMRY_FILE_TMPL     = `[commit]: # '{{.Commit}}'

test
`
)

var (
	projectRoot string
	sumryFile   string
	debugLog    bool
)

type SumryFileTemplateVars struct {
	Commit string
}

// this one goes to you Rob
func parseCommit(line string) (commit string, found bool) {
	regex := regexp.MustCompile(COMMIT_MAGIC_STRING)

	if match := regex.FindStringSubmatch(line); len(match) == 2 {
		commit = match[1]
		found = true
	}
	return
}

func headFile(fileName string) (string, error) {
	file, err := os.Open(fileName)
	if err != nil {
		return "", err
	}
	defer file.Close()

	scanner := bufio.NewScanner(file)
	if scanner.Scan() {
		return scanner.Text(), nil
	} else if err := scanner.Err(); err != nil {
		return "", err
	}
	return "", nil
}

func commitsSinceHash(prevCommit string, head *plumbing.Reference, repo *git.Repository) ([]*object.Commit, error) {
	commits := []*object.Commit{}

	cIter, err := repo.Log(&git.LogOptions{From: head.Hash(), Order: git.LogOrderCommitterTime})
	if err != nil {
		return commits, err
	}

	for {
		c, err := cIter.Next()
		if err != nil && err != io.EOF {
			return commits, err
		}
		if c == nil {
			break
		}
		commits = append(commits, c)
		if c.Hash.String() == prevCommit {
			break
		}
	}
	return commits, nil
}

func mustGetRepo(gitPath string, logger *log.Logger) *git.Repository {
	repo, err := git.PlainOpen(gitPath)
	if err != nil {
		logger.Fatal("failed to open git repo", "path", gitPath, "error", err)
	}
	return repo
}

func generateSumryFileText(commit string) string {
	output := bytes.Buffer{}
	vars := SumryFileTemplateVars{commit}
	tmpl, err := template.New("sumry").Parse(SUMRY_FILE_TMPL)
	if err != nil {
		log.Error("failed to parse template", "error", err)
		return ""
	}
	if err = tmpl.Execute(&output, vars); err != nil {
		log.Error("failed to execute the template", "error", err)
	}
	return output.String()
}

func init() {
	flag.StringVar(&projectRoot, "project-root", "", "the project root vergo will parse")
	flag.StringVar(&sumryFile, "sumry-file", "SUMRY.md", "the previous sumry output file")
	flag.BoolVar(&debugLog, "debug", false, "enable debug logs")

	flag.Parse()
}

func main() {
	if debugLog {
		log.SetLevel(log.DebugLevel)
	}

	log.Info("SUMRY")
	logger := log.With("project-root", projectRoot,
		"sumry-file", sumryFile)
	logger.Debug("config")

	fileName := fmt.Sprintf("%s/%s", projectRoot, sumryFile)
	firstLine, err := headFile(fileName)
	if err != nil {
		log.Fatal("failed to read file", "file", fileName, "error", err)
	}

	if commit, ok := parseCommit(firstLine); ok {
		logger.Info("Parsed previous log commit", "commit", commit)

		gitPath := fmt.Sprintf("%s/%s", projectRoot, ".git")
		repo := mustGetRepo(gitPath, logger)

		head, err := repo.Head()
		if err != nil {
			logger.Fatal("failed to get HEAD", "error", err)
		}
		logger.Debug("current HEAD", "ref", head.Hash())

		if commit == head.Hash().String() {
			logger.Warn("sumry file is already up-to-date")
			return
		}

		commits, err := commitsSinceHash(commit, head, repo)
		if err != nil {
			log.Fatal("failed to get commits since last update", "err", err)
		}

		fmt.Println(commits)

		logger.Debug("generating sumry file")
		fmt.Print(generateSumryFileText(head.Hash().String()))
	} else {
		logger.Error("failed to locate previous sumry file")
	}
}
