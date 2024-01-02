import * as cdk from 'aws-cdk-lib';
import { Construct } from 'constructs';
import * as s3 from 'aws-cdk-lib/aws-s3';
import * as ec2 from 'aws-cdk-lib/aws-ec2';
import * as ecr from 'aws-cdk-lib/aws-ecr';
import * as iam from 'aws-cdk-lib/aws-iam';

export class CombustionStack extends cdk.Stack {
  constructor(scope: Construct, id: string, props?: cdk.StackProps) {
    super(scope, id, props);

    // S3 Bucket
    const name = this.node.tryGetContext('name');
    if (!name) {
      throw new Error('"name" is required');
    }
    const bucketName = `${name}-combustion`;
    const bucket = new s3.Bucket(this, 'CombustionBucket', {
      bucketName: bucketName,
      versioned: true,
    });

    // IAM Role for EC2
    const ec2Role = new iam.Role(this, 'InstanceRole', {
      assumedBy: new iam.ServicePrincipal('ec2.amazonaws.com'),
    });
    ec2Role.addManagedPolicy(iam.ManagedPolicy.fromAwsManagedPolicyName('AmazonSSMManagedInstanceCore'));

    // Allow EC2 to read from S3
    bucket.grantRead(ec2Role);

    const imageRepo = new ecr.Repository(this, 'Repository', {
      repositoryName: name,
      emptyOnDelete: true,
      removalPolicy: cdk.RemovalPolicy.DESTROY,
    });
    imageRepo.grantPull(ec2Role);

    // EC2 Instance
    const vpc = new ec2.Vpc(this, 'Vpc', {
      maxAzs: 2, // Adjust based on your requirements
      natGateways: 0, // Don't need a NAT Gateway since my EC2 instance has public IP
    });

    const userData = ec2.UserData.forLinux();
    userData.addCommands(
      'yum update -y',
      'yum install docker -y',
      'service docker start',
      'sleep 10',
      'cat <<EOF > /tmp/update.sh',
      '#!/bin/bash',
      'set -euxo',
      `aws ecr get-login-password --region us-west-2 | docker login --username AWS --password-stdin ${imageRepo.repositoryUri}`,
      `docker pull ${imageRepo.repositoryUri}:latest`,
      `docker run -p 80:8080 -d ${imageRepo.repositoryUri}:latest ${bucketName}`,
      'EOF',
    );

    const securityGroup = new ec2.SecurityGroup(this, 'SecGroup', {
      vpc,
      securityGroupName: `${name}-secgroup`,
    });
    securityGroup.addIngressRule(ec2.Peer.anyIpv4(), ec2.Port.tcp(80), 'Allow HTTP traffic');

    const instance = new ec2.Instance(this, 'Instance', {
      vpc,
      securityGroup,
      associatePublicIpAddress: true,
      instanceType: ec2.InstanceType.of(ec2.InstanceClass.T2, ec2.InstanceSize.MICRO),
      machineImage: ec2.MachineImage.latestAmazonLinux2023(),
      role: ec2Role,
      vpcSubnets: { subnetType: ec2.SubnetType.PUBLIC },
      userData: userData,
    });

    // IAM User
    const user = new iam.User(this, 'User');
    user.addManagedPolicy(iam.ManagedPolicy.fromAwsManagedPolicyName('AmazonS3FullAccess'));


    // Output the S3 bucket name
    new cdk.CfnOutput(this, 'Output', {
      value: bucket.bucketName,
    });
  }
}
