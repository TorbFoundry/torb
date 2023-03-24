---
title: Overview
---

# {% $markdoc.frontmatter.title %}

## What is Torb?

Torb is a tool for quickly setting up best practice development infrastructure along with development stacks that have reasonably sane defaults. Instead of taking a couple hours to get a project started and then a week to get your infrastructure correct, do all of that in a couple minutes. Right now we have a CLI tool and we're in the process of building a hosted version that will deploy projects into your kubernetes cluster's on AWS, Azure, and GCP

## Mission

Make it simple and easy for software engineers to create and deploy infrastructure with best practices in place. The ideal would be 10 minutes or less to do so and the point where we consider Torb a failure is if it takes more than an hour to configure any one environment.

In addition to the above Torb needs to provide an easy way of adding additional infrastructure and requirements as a project scales. On day one you probably have logs and something like Sentry or Rollbar but you might not have great CI/CD or more complex distributed tracing or bill of materials or an artifact repository or whatever. It should be very simple to add these to an existing system. Infrastructure needs change as a project changes and we want to accomodate that for developers so they can focus more on core business features.
